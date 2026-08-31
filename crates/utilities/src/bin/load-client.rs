use clap::{Parser, Subcommand};
use dashmap::DashMap;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use gtpv2::GtpMessage;
use gtpv2::codec::GtpCodec;
use gtpv2::ie::ambr::Ambr;
use gtpv2::ie::apn::Apn;
use gtpv2::ie::bearer_context::BearerContext;
use gtpv2::ie::bearer_qos::BearerQos;
use gtpv2::ie::fteid::{Fteid, InterfaceType};
use gtpv2::ie::imsi::Imsi;
use gtpv2::ie::paa::Paa;
use gtpv2::ie::pdn_type::PdnType;
use gtpv2::ie::rat_type::RatType;
use gtpv2::ie::selection_mode::SelectionMode;
use gtpv2::messages::{
    CreateSessionRequest, CreateSessionResponse, DeleteSessionRequest, DeleteSessionResponse,
};
use std::net::{Ipv4Addr, SocketAddr};
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, oneshot};
use tokio::time::interval;
use tokio_util::udp::UdpFramed;

use crate::metrics::{Metrics, spawn_metrics_writer};

type PendingMap = Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>;
type GtpSink = SplitSink<UdpFramed<GtpCodec>, (GtpMessage, SocketAddr)>;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
    #[arg(long)]
    target: String,
    #[arg(long, default_value = "0.0.0.0:2123")]
    bind: String,
    #[arg(long, default_value_t = 30)]
    duration_secs: u64,
    #[arg(long, default_value_t = 1000)]
    timeout_ms: u64,
    #[arg(long, default_value = "001010000000001")]
    imsi_base: String,
    #[arg(long)]
    metrics_file: Option<String>,
    #[arg(long, default_value_t = 10)]
    metrics_interval_secs: u64,
}

#[derive(Subcommand)]
enum Mode {
    Open {
        #[arg(long, default_value_t = 100)]
        rate: u64,
    },
    Closed {
        #[arg(long, default_value_t = 50)]
        concurrency: u64,
        #[arg(long, default_value_t = false)]
        with_delete: bool,
        #[arg(long, value_parser = humantime::parse_duration)]
        session_hold_secs: Option<Duration>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.mode {
        Mode::Open { rate } => {
            run_open_loop(RunOpenLoopArgs {
                target: cli.target,
                bind: cli.bind,
                rate,
                duration_secs: cli.duration_secs,
                timeout_ms: cli.timeout_ms,
                imsi_base: cli.imsi_base,
                metrics_file: cli.metrics_file,
                metrics_interval_secs: cli.metrics_interval_secs,
            })
            .await
        }
        Mode::Closed {
            concurrency,
            with_delete,
            session_hold_secs,
        } => {
            run_closed_loop(RunClosedLoopArgs {
                target: cli.target,
                bind: cli.bind,
                concurrency,
                duration_secs: cli.duration_secs,
                timeout_ms: cli.timeout_ms,
                imsi_base: cli.imsi_base,
                with_delete,
                session_hold_duration: session_hold_secs,
                metrics_file: cli.metrics_file,
                metrics_interval_secs: cli.metrics_interval_secs,
            })
            .await
        }
    }
}

mod metrics {
    use hdrhistogram::Histogram;
    use std::io::Write;
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use tokio::sync::Mutex;
    use tokio::time::interval;

    pub struct Metrics {
        // Cumulative
        latencies: Histogram<u64>,
        sent: u64,
        received: u64,
        timeouts: u64,
        errors: u64,
        rejected: u64,

        // Interval-only
        interval_latencies: Histogram<u64>,
        interval_sent: u64,
        interval_received: u64,
        interval_timeouts: u64,
        interval_errors: u64,
        interval_rejected: u64,
    }

    struct WriteSnapshotArgs<'a, I>
    where
        I: Write,
    {
        writer: &'a mut I,
        now_unix: u64,
        throughput: f64,
        p50: f64,
        p90: f64,
        p99: f64,
        max: f64,
    }

    impl Metrics {
        pub fn new() -> Self {
            Metrics {
                latencies: Histogram::new_with_bounds(1, 60_000_000, 3).unwrap(),
                sent: 0,
                received: 0,
                timeouts: 0,
                errors: 0,
                rejected: 0,
                interval_latencies: Histogram::new_with_bounds(1, 60_000_000, 3).unwrap(),
                interval_sent: 0,
                interval_received: 0,
                interval_timeouts: 0,
                interval_errors: 0,
                interval_rejected: 0,
            }
        }

        pub fn record_latency(&mut self, d: Duration) {
            let micros = d.as_micros() as u64;
            let _ = self.latencies.record(micros);
            let _ = self.interval_latencies.record(micros);
        }

        pub fn increment_sent(&mut self) {
            self.sent += 1;
            self.interval_sent += 1;
        }

        pub fn increment_received(&mut self) {
            self.received += 1;
            self.interval_received += 1;
        }

        pub fn increment_timeout(&mut self) {
            self.timeouts += 1;
            self.interval_timeouts += 1;
        }

        pub fn increment_error(&mut self) {
            self.errors += 1;
            self.interval_errors += 1;
        }

        pub fn increment_rejected(&mut self) {
            self.rejected += 1;
            self.interval_rejected += 1;
        }

        fn snapshot_and_reset(
            &mut self,
            writer: &mut impl Write,
            elapsed_secs: f64,
        ) -> anyhow::Result<()> {
            let secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let now_unix = secs;
            let throughput = self.interval_received as f64 / elapsed_secs.max(0.001);

            let (p50, p90, p99, max) = self.get_percentiles();

            self.write_snapshot(WriteSnapshotArgs {
                writer,
                now_unix,
                throughput,
                p50,
                p90,
                p99,
                max,
            })?;

            self.reset_interval_counters();

            Ok(())
        }

        fn write_snapshot<I: Write>(
            &mut self,
            args: WriteSnapshotArgs<'_, I>,
        ) -> Result<(), anyhow::Error> {
            writeln!(
                args.writer,
                "{},{},{},{},{},{},{:.2},{:.3},{:.3},{:.3},{:.3}",
                args.now_unix,
                self.interval_sent,
                self.interval_received,
                self.interval_timeouts,
                self.interval_errors,
                self.interval_rejected,
                args.throughput,
                args.p50,
                args.p90,
                args.p99,
                args.max,
            )?;

            args.writer.flush()?;

            Ok(())
        }

        fn get_percentiles(&mut self) -> (f64, f64, f64, f64) {
            if !self.interval_latencies.is_empty() {
                (
                    self.interval_latencies.value_at_quantile(0.50) as f64 / 1000.0,
                    self.interval_latencies.value_at_quantile(0.90) as f64 / 1000.0,
                    self.interval_latencies.value_at_quantile(0.99) as f64 / 1000.0,
                    self.interval_latencies.max() as f64 / 1000.0,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            }
        }

        fn reset_interval_counters(&mut self) {
            self.interval_latencies.reset();
            self.interval_sent = 0;
            self.interval_received = 0;
            self.interval_timeouts = 0;
            self.interval_errors = 0;
            self.interval_rejected = 0;
        }

        pub fn print_summary(&self, wall_time: Duration) {
            println!("\n=== Load Test Summary ===");
            println!("Sent:      {}", self.sent);
            println!("Received:  {}", self.received);
            println!("Timeouts:  {}", self.timeouts);
            println!("Errors:    {}", self.errors);
            println!("Rejected:  {}", self.rejected);
            let throughput = self.received as f64 / wall_time.as_secs_f64();
            println!("Throughput: {throughput:.1} resp/sec");

            if !self.latencies.is_empty() {
                println!("\nLatency (ms):");
                println!(
                    "  p50: {:.2}",
                    self.latencies.value_at_quantile(0.50) as f64 / 1000.0
                );
                println!(
                    "  p90: {:.2}",
                    self.latencies.value_at_quantile(0.90) as f64 / 1000.0
                );
                println!(
                    "  p99: {:.2}",
                    self.latencies.value_at_quantile(0.99) as f64 / 1000.0
                );
                println!("  Max: {:.2}", self.latencies.max() as f64 / 1000.0);
            }
        }
    }

    fn write_csv_header(writer: &mut impl Write) -> anyhow::Result<()> {
        writeln!(
            writer,
            "unix_time,sent,received,timeouts,errors,rejected,throughput_per_sec,p50_ms,p90_ms,p99_ms,max_ms"
        )?;

        writer.flush()?;

        Ok(())
    }

    pub fn spawn_metrics_writer(
        metrics: Arc<Mutex<Metrics>>,
        output_path: String,
        interval_secs: u64,
        deadline: Instant,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let file = match std::fs::File::create(&output_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Failed to create metrics file {output_path}: {e}");
                    return;
                }
            };

            let mut writer = std::io::BufWriter::new(file);
            if let Err(e) = write_csv_header(&mut writer) {
                eprintln!("Failed to write CSV header: {e}");
                return;
            }

            let mut ticker = interval(Duration::from_secs(interval_secs));
            ticker.tick().await;

            loop {
                ticker.tick().await;

                let now = Instant::now();
                let mut metrics_guard = metrics.lock().await;

                if let Err(e) = metrics_guard.snapshot_and_reset(&mut writer, interval_secs as f64)
                {
                    eprintln!("Metrics snapshot write error: {e}");
                }

                drop(metrics_guard);

                if now >= deadline {
                    break;
                }
            }
        })
    }
}

fn determine_local_ip(target: SocketAddr) -> anyhow::Result<Ipv4Addr> {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0")?;

    probe.connect(target)?;

    match probe.local_addr()?.ip() {
        std::net::IpAddr::V4(v4) => Ok(v4),
        std::net::IpAddr::V6(_) => anyhow::bail!(
            "target resolved to IPv6; this application only supports IPv4 F-TEID currently"
        ),
    }
}

fn derive_imsi(base: &str, n: u64) -> String {
    let mut digits: Vec<u8> = base.bytes().collect();
    let suffix = format!("{n:09}");
    let start = digits.len() - suffix.len();

    digits[start..].copy_from_slice(suffix.as_bytes());

    String::from_utf8(digits).unwrap()
}

fn build_create_session_request(imsi: String, seq: u32, local_ip: Ipv4Addr) -> GtpMessage {
    CreateSessionRequest {
        imsi: Imsi(imsi),
        sender_fteid: Fteid {
            interface_type: InterfaceType::S5S8SgwGtpC.into(),
            teid_or_gre_key: seq,
            ipv4: Some(local_ip),
            ipv6: None,
        },
        apn: Apn("internet".to_string()),
        ambr: Ambr {
            uplink_kbps: 50_000,
            downlink_kbps: 100_000,
        },
        rat_type: RatType::Eutran,
        pdn_type: PdnType::Ipv4,
        paa: Paa::request_v4(),
        selection_mode: SelectionMode::MsOrNetworkProvidedSubscriptionVerified,
        bearer_context: BearerContext {
            ebi: Some(5),
            cause: None,
            fteid: Some(Fteid {
                interface_type: InterfaceType::S5S8SgwGtpU.into(),
                teid_or_gre_key: seq,
                ipv4: Some(local_ip),
                ipv6: None,
            }),
            bearer_qos: Some(BearerQos {
                max_bitrate_uplink_kbps: 50_000,
                max_bitrate_downlink_kbps: 100_000,
                ..Default::default()
            }),
        },
        sequence_number: seq,
    }
    .into()
}

fn spawn_response_dispatcher(
    mut stream: futures::stream::SplitStream<UdpFramed<GtpCodec>>,
    pending: PendingMap,
    metrics: Arc<Mutex<Metrics>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(result) = stream.next().await {
            let received_at = Instant::now();

            match result {
                Ok((message, _address)) => {
                    let sequence_number = message.header.sequence_number;
                    if let Some((_, tx)) = pending.remove(&sequence_number) {
                        let _ = tx.send((message, received_at));
                    }
                }
                Err(e) => {
                    metrics.lock().await.increment_error();
                    eprintln!("Decode error: {e}");
                }
            }
        }
    })
}

struct RunOpenLoopArgs {
    target: String,
    bind: String,
    rate: u64,
    duration_secs: u64,
    timeout_ms: u64,
    imsi_base: String,
    metrics_file: Option<String>,
    metrics_interval_secs: u64,
}

async fn run_open_loop(args: RunOpenLoopArgs) -> anyhow::Result<()> {
    let target: SocketAddr = args.target.parse()?;
    let deadline = Instant::now() + Duration::from_secs(args.duration_secs);
    let local_ip = determine_local_ip(target)?;

    let socket = UdpSocket::bind(&args.bind).await?;

    println!(
        "Client bound on {}, advertising F-TEID IP {local_ip}, sending to {target}",
        socket.local_addr()?
    );

    let framed = UdpFramed::new(socket, GtpCodec);
    let (sink, stream) = framed.split();
    let sink: Arc<Mutex<GtpSink>> = Arc::new(Mutex::new(sink));

    let pending: PendingMap = Arc::new(DashMap::new());
    let sequence_counter = Arc::new(AtomicU32::new(1));
    let metrics = Arc::new(Mutex::new(Metrics::new()));

    let metrics_task = args.metrics_file.map(|path| {
        spawn_metrics_writer(metrics.clone(), path, args.metrics_interval_secs, deadline)
    });

    spawn_response_dispatcher(stream, pending.clone(), metrics.clone());

    let mut ticker = interval(Duration::from_secs_f64(1.0 / args.rate as f64));
    let mut generator_handles = Vec::new();
    let mut counter: u64 = 0;

    while Instant::now() < deadline {
        generate_packets(GeneratePacketsArgs {
            timeout_ms: args.timeout_ms,
            imsi_base: &args.imsi_base,
            target,
            local_ip,
            sink: &sink,
            pending: &pending,
            sequence_counter: &sequence_counter,
            metrics: &metrics,
            ticker: &mut ticker,
            generator_handles: &mut generator_handles,
            counter: &mut counter,
        })
        .await;
    }

    let wall_start = Instant::now() - Duration::from_secs(args.duration_secs);

    for handle in generator_handles {
        let _ = handle.await;
    }

    if let Some(task) = metrics_task {
        task.abort();
    }

    metrics.lock().await.print_summary(wall_start.elapsed());

    Ok(())
}

struct GeneratePacketsArgs<'a> {
    timeout_ms: u64,
    imsi_base: &'a str,
    target: SocketAddr,
    local_ip: Ipv4Addr,
    sink: &'a Arc<Mutex<GtpSink>>,
    pending: &'a Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>,
    sequence_counter: &'a Arc<AtomicU32>,
    metrics: &'a Arc<Mutex<Metrics>>,
    ticker: &'a mut tokio::time::Interval,
    generator_handles: &'a mut Vec<tokio::task::JoinHandle<()>>,
    counter: &'a mut u64,
}

async fn generate_packets(args: GeneratePacketsArgs<'_>) {
    args.ticker.tick().await;
    *args.counter += 1;

    let sequence_number = args.sequence_counter.fetch_add(1, AtomicOrdering::Relaxed) & 0x00FF_FFFF;
    let imsi = derive_imsi(args.imsi_base, *args.counter);
    let req = build_create_session_request(imsi, sequence_number, args.local_ip);
    let (tx, rx) = oneshot::channel();

    args.pending.insert(sequence_number, tx);

    let sink = args.sink.clone();
    let metrics = args.metrics.clone();
    let pending_cleanup = args.pending.clone();
    let sent_at = Instant::now();

    args.generator_handles.push(tokio::spawn(async move {
        if let ControlFlow::Break(_) = generator(GeneratorArgs {
            timeout_ms: args.timeout_ms,
            target: args.target,
            sequence_number,
            req,
            rx,
            sink,
            metrics,
            pending_cleanup,
            sent_at,
        })
        .await
        {}
    }));
}

struct GeneratorArgs {
    timeout_ms: u64,
    target: SocketAddr,
    sequence_number: u32,
    req: GtpMessage,
    rx: oneshot::Receiver<(GtpMessage, Instant)>,
    sink: Arc<Mutex<GtpSink>>,
    metrics: Arc<Mutex<Metrics>>,
    pending_cleanup: Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>,
    sent_at: Instant,
}

async fn generator(args: GeneratorArgs) -> ControlFlow<()> {
    if let Some(value) = send_message(
        args.target,
        args.sequence_number,
        args.req,
        args.sink,
        &args.metrics,
        &args.pending_cleanup,
    )
    .await
    {
        return value;
    }

    args.metrics.lock().await.increment_sent();

    match tokio::time::timeout(Duration::from_millis(args.timeout_ms), args.rx).await {
        Ok(Ok((message, received_at))) => {
            receive_create_session_response(
                args.sequence_number,
                &args.metrics,
                args.sent_at,
                message,
                received_at,
            )
            .await;
        }
        Ok(Err(_)) => {
            args.metrics.lock().await.increment_error();
        }
        Err(_) => {
            args.metrics.lock().await.increment_timeout();
            args.pending_cleanup.remove(&args.sequence_number);
        }
    }

    ControlFlow::Continue(())
}

async fn receive_create_session_response(
    sequence_number: u32,
    metrics: &Arc<Mutex<Metrics>>,
    sent_at: Instant,
    message: GtpMessage,
    received_at: Instant,
) {
    let mut metric = metrics.lock().await;

    metric.increment_received();
    metric.record_latency(received_at - sent_at);

    drop(metric);

    if let Ok(resp) = CreateSessionResponse::try_from(&message)
        && !resp.cause.is_accepted()
    {
        metrics.lock().await.increment_rejected();

        eprintln!(
            "Sequence number {sequence_number}: rejected, cause={}",
            resp.cause.value
        );
    }
}

async fn send_message(
    target: SocketAddr,
    sequence_number: u32,
    request: GtpMessage,
    sink: Arc<Mutex<GtpSink>>,
    metrics: &Arc<Mutex<Metrics>>,
    pending_cleanup: &Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>,
) -> Option<ControlFlow<()>> {
    let mut sink = sink.lock().await;

    if let Err(e) = sink.send((request, target)).await {
        metrics.lock().await.increment_error();

        eprintln!("Send error: {e}");

        pending_cleanup.remove(&sequence_number);

        return Some(ControlFlow::Break(()));
    }

    None
}

struct RunClosedLoopArgs {
    target: String,
    bind: String,
    concurrency: u64,
    duration_secs: u64,
    timeout_ms: u64,
    imsi_base: String,
    with_delete: bool,
    session_hold_duration: Option<Duration>,
    metrics_file: Option<String>,
    metrics_interval_secs: u64,
}

async fn run_closed_loop(args: RunClosedLoopArgs) -> anyhow::Result<()> {
    let target: SocketAddr = args.target.parse()?;
    let deadline = Instant::now() + Duration::from_secs(args.duration_secs);
    let local_ip = determine_local_ip(target)?;

    let socket = UdpSocket::bind(&args.bind).await?;
    println!(
        "Closed-loop client bound on {}, advertising F-TEID IP {local_ip}, sending to {target}",
        socket.local_addr()?
    );

    let framed = UdpFramed::new(socket, GtpCodec);
    let (sink, stream) = framed.split();
    let sink: Arc<Mutex<GtpSink>> = Arc::new(Mutex::new(sink));

    let pending: PendingMap = Arc::new(DashMap::new());
    let sequence_counter = Arc::new(AtomicU32::new(1));
    let imsi_counter = Arc::new(AtomicU64::new(1));
    let metrics = Arc::new(Mutex::new(Metrics::new()));

    let metrics_task = args.metrics_file.map(|path| {
        spawn_metrics_writer(metrics.clone(), path, args.metrics_interval_secs, deadline)
    });

    spawn_response_dispatcher(stream, pending.clone(), metrics.clone());

    let mut workers = Vec::new();
    for worker_id in 0..args.concurrency {
        let sink = sink.clone();
        let pending = pending.clone();
        let sequence_counter = sequence_counter.clone();
        let imsi_counter = imsi_counter.clone();
        let imsi_base = args.imsi_base.clone();
        let metrics = metrics.clone();

        workers.push(tokio::spawn(async move {
            if let Err(e) = closed_loop_worker(ClosedLoopWorkerArgs {
                worker_id,
                target,
                deadline,
                timeout_ms: args.timeout_ms,
                imsi_base,
                local_ip,
                sequence_counter,
                imsi_counter,
                sink,
                pending,
                metrics,
                with_delete: args.with_delete,
                session_hold_duration: args.session_hold_duration,
            })
            .await
            {
                eprintln!("Worker {worker_id} error: {e}");
            }
        }));
    }

    let wall_start = Instant::now();

    for worker in workers {
        let _ = worker.await;
    }

    if let Some(task) = metrics_task {
        task.abort();
    }

    metrics.lock().await.print_summary(wall_start.elapsed());

    Ok(())
}

struct ClosedLoopWorkerArgs {
    worker_id: u64,
    target: SocketAddr,
    deadline: Instant,
    timeout_ms: u64,
    imsi_base: String,
    local_ip: Ipv4Addr,
    sequence_counter: Arc<AtomicU32>,
    imsi_counter: Arc<AtomicU64>,
    sink: Arc<Mutex<GtpSink>>,
    pending: PendingMap,
    metrics: Arc<Mutex<Metrics>>,
    with_delete: bool,
    session_hold_duration: Option<Duration>,
}

async fn closed_loop_worker(args: ClosedLoopWorkerArgs) -> anyhow::Result<()> {
    while Instant::now() < args.deadline {
        if let ControlFlow::Break(_) = do_worker_work(DoWorkerWorkArgs {
            worker_id: args.worker_id,
            target: args.target,
            timeout_ms: args.timeout_ms,
            imsi_base: &args.imsi_base,
            local_ip: args.local_ip,
            sequence_counter: &args.sequence_counter,
            imsi_counter: &args.imsi_counter,
            sink: &args.sink,
            pending: &args.pending,
            metrics: &args.metrics,
            with_delete: args.with_delete,
            session_hold_duration: args.session_hold_duration,
        })
        .await
        {
            continue;
        }
    }

    Ok(())
}

struct DoWorkerWorkArgs<'a> {
    worker_id: u64,
    target: SocketAddr,
    timeout_ms: u64,
    imsi_base: &'a str,
    local_ip: Ipv4Addr,
    sequence_counter: &'a Arc<AtomicU32>,
    imsi_counter: &'a Arc<AtomicU64>,
    sink: &'a Arc<Mutex<GtpSink>>,
    pending: &'a Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>,
    metrics: &'a Arc<Mutex<Metrics>>,
    with_delete: bool,
    session_hold_duration: Option<Duration>,
}

async fn do_worker_work(args: DoWorkerWorkArgs<'_>) -> ControlFlow<()> {
    let sequence_number = args.sequence_counter.fetch_add(1, AtomicOrdering::Relaxed) & 0x00FF_FFFF;
    let n = args.imsi_counter.fetch_add(1, AtomicOrdering::Relaxed);
    let imsi = derive_imsi(args.imsi_base, n);
    let request = build_create_session_request(imsi, sequence_number, args.local_ip);
    let (send, receive) = oneshot::channel();

    args.pending.insert(sequence_number, send);

    let sent_at = Instant::now();

    if let Some(value) = worker_send_message(
        args.worker_id,
        args.target,
        args.sink,
        args.pending,
        args.metrics,
        sequence_number,
        request,
    )
    .await
    {
        return value;
    }

    args.metrics.lock().await.increment_sent();

    let create_session_response = match create_session_response(
        args.worker_id,
        args.timeout_ms,
        args.pending,
        args.metrics,
        sequence_number,
        receive,
        sent_at,
    )
    .await
    {
        Ok(value) => value,
        Err(value) => return value,
    };

    if !create_session_response.cause.is_accepted() {
        args.metrics.lock().await.increment_rejected();

        eprintln!(
            "Worker {}: rejected, cause={}",
            args.worker_id, create_session_response.cause.value
        );

        return ControlFlow::Break(());
    }

    if !args.with_delete {
        return ControlFlow::Break(());
    }

    if let Some(hold) = args.session_hold_duration {
        tokio::time::sleep(hold).await;
    }

    let delete_session_request_sequence_number =
        args.sequence_counter.fetch_add(1, AtomicOrdering::Relaxed) & 0x00FF_FFFF;

    let delete_session_request = DeleteSessionRequest {
        linked_ebi: create_session_response.bearer_context.ebi.unwrap_or(5),
        teid: create_session_response.teid,
        sequence_number: delete_session_request_sequence_number,
    };

    let (send, receive) = oneshot::channel();

    args.pending
        .insert(delete_session_request_sequence_number, send);

    let delete_session_request_sent_at = Instant::now();

    if let Some(value) = worker_send_message(
        args.worker_id,
        args.target,
        args.sink,
        args.pending,
        args.metrics,
        delete_session_request_sequence_number,
        delete_session_request,
    )
    .await
    {
        return value;
    }

    args.metrics.lock().await.increment_sent();

    match tokio::time::timeout(Duration::from_millis(args.timeout_ms), receive).await {
        Ok(Ok((message, received_at))) => {
            worker_receive_delete_session_response(
                args.worker_id,
                sequence_number,
                args.metrics,
                delete_session_request_sent_at,
                message,
                received_at,
            )
            .await;
        }
        Ok(Err(_)) => {
            args.metrics.lock().await.increment_error();
        }
        Err(_) => {
            args.metrics.lock().await.increment_timeout();
            args.pending.remove(&delete_session_request_sequence_number);
        }
    }

    ControlFlow::Continue(())
}

async fn create_session_response(
    worker_id: u64,
    timeout_ms: u64,
    pending: &Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>,
    metrics: &Arc<Mutex<Metrics>>,
    sequence_number: u32,
    rx: oneshot::Receiver<(GtpMessage, Instant)>,
    sent_at: Instant,
) -> Result<CreateSessionResponse, ControlFlow<()>> {
    match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
        Ok(Ok((message, received_at))) => {
            match worker_receive_create_session_response(
                worker_id,
                sequence_number,
                metrics,
                sent_at,
                message,
                received_at,
            )
            .await
            {
                Ok(value) => Ok(value),
                Err(value) => Err(value),
            }
        }
        Ok(Err(_)) => {
            metrics.lock().await.increment_error();
            Err(ControlFlow::Break(()))
        }
        Err(_) => {
            metrics.lock().await.increment_timeout();
            pending.remove(&sequence_number);
            Err(ControlFlow::Break(()))
        }
    }
}

async fn worker_receive_delete_session_response(
    worker_id: u64,
    sequence_number: u32,
    metrics: &Arc<Mutex<Metrics>>,
    sent_at: Instant,
    message: GtpMessage,
    received_at: Instant,
) {
    let mut m = metrics.lock().await;

    m.increment_received();
    m.record_latency(received_at - sent_at);

    drop(m);

    if let Ok(response) = DeleteSessionResponse::try_from(&message)
        && !response.cause.is_accepted()
    {
        metrics.lock().await.increment_rejected();

        eprintln!(
            "Worker {worker_id}, sequence number {sequence_number}: delete rejected, cause={}",
            response.cause.value
        );
    }
}

async fn worker_receive_create_session_response(
    worker_id: u64,
    sequence_number: u32,
    metrics: &Arc<Mutex<Metrics>>,
    sent_at: Instant,
    message: GtpMessage,
    received_at: Instant,
) -> Result<CreateSessionResponse, ControlFlow<()>> {
    let mut metrics = metrics.lock().await;

    metrics.increment_received();
    metrics.record_latency(received_at - sent_at);

    drop(metrics);

    Ok(match CreateSessionResponse::try_from(&message) {
        Ok(response) => response,
        Err(error) => {
            eprintln!(
                "Worker {worker_id}, sequence number {sequence_number}: bad Create Session Response: {error}"
            );

            return Err(ControlFlow::Break(()));
        }
    })
}

async fn worker_send_message(
    worker_id: u64,
    target: SocketAddr,
    sink: &Arc<Mutex<GtpSink>>,
    pending: &Arc<DashMap<u32, oneshot::Sender<(GtpMessage, Instant)>>>,
    metrics: &Arc<Mutex<Metrics>>,
    sequence_number: u32,
    request: impl Into<GtpMessage>,
) -> Option<ControlFlow<()>> {
    let mut sink = sink.lock().await;

    if let Err(e) = sink.send((request.into(), target)).await {
        metrics.lock().await.increment_error();

        eprintln!("Worker {worker_id}: send error: {e}");

        pending.remove(&sequence_number);

        return Some(ControlFlow::Break(()));
    }

    None
}

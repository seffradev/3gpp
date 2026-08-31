use clap::Parser;
use futures::{SinkExt, StreamExt};
use gtpv2::{
    codec::GtpCodec,
    ie::{
        ambr::Ambr,
        bearer_context::BearerContext,
        cause::Cause,
        fteid::{Fteid, InterfaceType},
        recovery::Recovery,
        types::message_type,
    },
    messages::{
        CreateSessionRequest, CreateSessionResponse, DeleteSessionRequest, DeleteSessionResponse,
        EchoRequest, EchoResponse,
    },
};
use std::{
    net::Ipv4Addr,
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "0.0.0.0:2123")]
    bind: String,
    #[arg(long)]
    advertise_ip: std::net::Ipv4Addr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli.bind, cli.advertise_ip).await
}

pub async fn run(bind: String, advertise_ip: Ipv4Addr) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(&bind).await?;

    println!("Load server listening on {bind}");

    let mut framed = UdpFramed::new(socket, GtpCodec);
    let mut count: u64 = 0;

    static NEXT_TEID: AtomicU32 = AtomicU32::new(1000);

    let restart_counter = get_restart_counter();

    while let Some(result) = framed.next().await {
        handle_message(
            advertise_ip,
            &mut framed,
            &mut count,
            &NEXT_TEID,
            restart_counter,
            result,
        )
        .await;
    }

    Ok(())
}

async fn handle_message(
    advertise_ip: Ipv4Addr,
    framed: &mut UdpFramed<GtpCodec>,
    count: &mut u64,
    next_teid: &AtomicU32,
    restart_counter: u8,
    result: Result<(gtpv2::GtpMessage, std::net::SocketAddr), gtpv2::GtpError>,
) {
    match result {
        Ok((message, address)) => match message.header.message_type {
            message_type::ECHO_REQUEST => {
                *count += 1;
                handle_echo_request(framed, restart_counter, &message, address).await
            }
            message_type::DELETE_SESSION_REQUEST => {
                *count += 1;
                handle_delete_session_request(framed, &message, address).await
            }
            message_type::CREATE_SESSION_REQUEST => {
                *count += 1;
                handle_create_session_request(advertise_ip, framed, next_teid, message, address)
                    .await
            }
            other => {
                eprintln!("Unhandled message type {other} from {address}");
                return;
            }
        },
        Err(error) => eprintln!("Decode error: {error}"),
    }

    if count.is_multiple_of(1000) {
        println!("Handled {count} requests");
    }
}

async fn handle_create_session_request(
    advertise_ip: Ipv4Addr,
    framed: &mut UdpFramed<GtpCodec>,
    next_teid: &AtomicU32,
    message: gtpv2::GtpMessage,
    address: std::net::SocketAddr,
) {
    let allocated_teid = next_teid.fetch_add(1, Ordering::Relaxed);

    let create_session_request = match CreateSessionRequest::try_from(&message) {
        Ok(c) => c,
        Err(error) => {
            eprintln!("Failed to parse Create Session Request from {address}: {error}");
            return;
        }
    };

    let response = CreateSessionResponse {
        cause: Cause { value: 16 },
        sender_fteid: Fteid {
            interface_type: InterfaceType::S5S8PgwGtpC.into(),
            teid_or_gre_key: allocated_teid,
            ipv4: Some(advertise_ip),
            ipv6: None,
        },
        ambr: Some(Ambr {
            uplink_kbps: 50_000,
            downlink_kbps: 100_000,
        }),
        bearer_context: BearerContext {
            ebi: create_session_request.bearer_context.ebi,
            cause: Some(Cause { value: 16 }),
            fteid: Some(Fteid {
                interface_type: InterfaceType::S5S8PgwGtpU.into(),
                teid_or_gre_key: allocated_teid,
                ipv4: Some(advertise_ip),
                ipv6: None,
            }),
            ..Default::default()
        },
        teid: create_session_request.sender_fteid.teid_or_gre_key,
        sequence_number: create_session_request.sequence_number,
    };

    if let Err(error) = framed.send((response.into(), address)).await {
        eprintln!("Send error to {address}: {error}");
    }
}

async fn handle_delete_session_request(
    framed: &mut UdpFramed<GtpCodec>,
    message: &gtpv2::GtpMessage,
    address: std::net::SocketAddr,
) {
    let delete_session_request = match DeleteSessionRequest::try_from(message) {
        Ok(delete_session_request) => delete_session_request,
        Err(error) => {
            eprintln!("Failed to parse Delete Session Request from {address}: {error}");
            return;
        }
    };

    let response = DeleteSessionResponse {
        cause: Cause { value: 16 },
        teid: delete_session_request.teid,
        sequence_number: delete_session_request.sequence_number,
    };

    if let Err(error) = framed.send((response.into(), address)).await {
        eprintln!("Send error to {address}: {error}");
    }
}

async fn handle_echo_request(
    framed: &mut UdpFramed<GtpCodec>,
    restart_counter: u8,
    message: &gtpv2::GtpMessage,
    address: std::net::SocketAddr,
) {
    let echo_request = match EchoRequest::try_from(message) {
        Ok(error) => error,
        Err(error) => {
            eprintln!("Failed to parse Echo Request from {address}: {error}");
            return;
        }
    };

    let response = EchoResponse {
        recovery: Recovery { restart_counter },
        sequence_number: echo_request.sequence_number,
    };

    if let Err(error) = framed.send((response.into(), address)).await {
        eprintln!("Send error to {address}: {error}");
    }
}

fn get_restart_counter() -> u8 {
    let restart_counter: u8 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u8;

    restart_counter
}

#[cfg(windows)]
use std::str;
use std::{io, net::{self, ToSocketAddrs}};

pub async fn connector(addr: String, instance_name: Option<String>) -> tiberius::Result<smol::Async<net::TcpStream>> 
{
    let mut addr = addr.to_socket_addrs()?.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not resolve server host.")
    })?;

    if let Some(ref instance_name) = instance_name {
        addr = find_tcp_port(addr, instance_name).await?;
    };

    let mut stream = smol::Async::<net::TcpStream>::connect(addr).await?;
    stream.get_mut().set_nodelay(true)?;

    Ok(stream)
}

pub type Client = tiberius::Client<smol::Async<net::TcpStream>>;

/// This feature is not used on platforms other than Windows
#[cfg(not(windows))]
async fn find_tcp_port(addr: std::net::SocketAddr, _: &str) -> tiberius::Result<std::net::SocketAddr> {
    Ok(addr)
}

/// Use the SQL Browser to find the correct TCP port for the server
/// instance.
#[cfg(windows)]
async fn find_tcp_port(mut addr: std::net::SocketAddr, instance_name: &str) -> tiberius::Result<std::net::SocketAddr> {
    // First resolve the instance to a port via the
    // SSRP protocol/MS-SQLR protocol [1]
    // [1] https://msdn.microsoft.com/en-us/library/cc219703.aspx

    let local_bind: std::net::SocketAddr = if addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let msg = [&[4u8], instance_name.as_bytes()].concat();
    let mut buf = vec![0u8; 4096];

    let socket = smol::Async::<net::UdpSocket>::bind(&local_bind)?;
    socket.send_to(&msg, addr).await?;

    //let timeout = std::time::Duration::from_millis(1000);

    let len = socket.recv(&mut buf).await?;
//    let len = time::timeout(timeout, socket.recv(&mut buf))
//        .map_err(|_: time::Elapsed| {
//            tiberius::Error::Conversion(
//                format!(
//                    "SQL browser timeout during resolving instance {}",
//                    instance_name
//                )
//                .into(),
//            )
//        }).await??;


    buf.truncate(len);

    let err = tiberius::Error::Conversion(
        format!("Could not resolve SQL browser instance {}", instance_name).into(),
    );

    if len == 0 {
        return Err(err);
    }

    let response = str::from_utf8(&buf[3..len])?;

    let port: u16 = response
        .find("tcp;")
        .and_then(|pos| response[pos..].split(';').nth(1))
        .ok_or(err)
        .and_then(|val| Ok(val.parse()?))?;

    addr.set_port(port);

    Ok(addr)
}
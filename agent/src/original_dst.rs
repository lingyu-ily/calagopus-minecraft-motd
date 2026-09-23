use std::net::SocketAddr;
#[cfg(unix)]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::net::TcpStream;

#[cfg(unix)]
pub fn original_destination(stream: &TcpStream) -> std::io::Result<SocketAddr> {
    use std::{mem, os::fd::AsRawFd};

    unsafe fn read_sockaddr(
        fd: libc::c_int,
        level: libc::c_int,
        option: libc::c_int,
    ) -> std::io::Result<libc::sockaddr_storage> {
        let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        let mut length = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                fd,
                level,
                option,
                &mut storage as *mut _ as *mut libc::c_void,
                &mut length,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(storage)
    }

    fn convert(storage: &libc::sockaddr_storage) -> Option<SocketAddr> {
        match storage.ss_family as libc::c_int {
            libc::AF_INET => {
                let address = unsafe { &*(storage as *const _ as *const libc::sockaddr_in) };
                Some(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::from(u32::from_be(address.sin_addr.s_addr))),
                    u16::from_be(address.sin_port),
                ))
            }
            libc::AF_INET6 => {
                let address = unsafe { &*(storage as *const _ as *const libc::sockaddr_in6) };
                Some(SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::from(address.sin6_addr.s6_addr)),
                    u16::from_be(address.sin6_port),
                ))
            }
            _ => None,
        }
    }

    const SO_ORIGINAL_DST: libc::c_int = 80;
    const IP6T_SO_ORIGINAL_DST: libc::c_int = 80;
    let fd = stream.as_raw_fd();

    if let Ok(storage) = unsafe { read_sockaddr(fd, libc::SOL_IP, SO_ORIGINAL_DST) }
        && let Some(address) = convert(&storage)
    {
        return Ok(address);
    }
    let storage = unsafe { read_sockaddr(fd, libc::SOL_IPV6, IP6T_SO_ORIGINAL_DST) }?;
    convert(&storage).ok_or_else(|| std::io::Error::other("unsupported original destination"))
}

#[cfg(not(unix))]
pub fn original_destination(stream: &TcpStream) -> std::io::Result<SocketAddr> {
    stream.local_addr()
}

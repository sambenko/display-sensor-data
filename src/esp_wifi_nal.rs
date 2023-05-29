use esp_wifi::wifi_interface::Socket;
use esp_wifi::wifi_interface::WifiStack;
use esp_wifi::wifi_interface::WifiStackError;

use embedded_io::blocking::{Read, Write};
use smoltcp::wire::{IpAddress, Ipv4Address};
pub use embedded_nal;
use embedded_nal::TcpClientStack;

pub struct WifiTcpClientStack<'s, 'a: 's> {
    wifi_stack: &'s mut WifiStack<'s>,
    rx_buffer: &'a mut [u8],
    tx_buffer: &'a mut [u8],
}

impl<'s, 'a: 's> TcpClientStack for WifiTcpClientStack<'s, 'a> {
    type TcpSocket = Socket<'s, 'a>;
    type Error = WifiStackError;

    fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
        Ok(self.wifi_stack.get_socket(self.rx_buffer, self.tx_buffer))
    }

    fn connect(
        &mut self,
        socket: &mut Self::TcpSocket,
        remote: embedded_nal::SocketAddr,
    ) -> embedded_nal::nb::Result<(), Self::Error> {
        let remote_ip = match remote.ip() {
            embedded_nal::IpAddr::V4(ip) => {
                let octets = ip.octets();
                smoltcp::wire::IpAddress::Ipv4(
                    smoltcp::wire::Ipv4Address::new(octets[0], octets[1], octets[2], octets[3]),
                )
            }
            embedded_nal::IpAddr::V6(_) => unimplemented!(),
        };
        let remote_port = remote.port();

        match socket.open(remote_ip, remote_port) {
            Ok(()) => Ok(()),
            Err(e) => Err(embedded_nal::nb::Error::WouldBlock),
        }
    }

    fn is_connected(&mut self, socket: &Self::TcpSocket) -> Result<bool, Self::Error> {
        Ok(socket.is_connected())
    }

    fn send(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &[u8],
    ) -> embedded_nal::nb::Result<usize, Self::Error> {
        match socket.write(buffer) {
            Ok(n) => Ok(n),
            Err(e) => Err(embedded_nal::nb::Error::WouldBlock),
        }
    }

    fn receive(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &mut [u8],
    ) -> embedded_nal::nb::Result<usize, Self::Error> {
        match socket.read(buffer) {
            Ok(n) => Ok(n),
            Err(e) => Err(embedded_nal::nb::Error::WouldBlock),
        }
    }

    fn close(&mut self, socket: Self::TcpSocket) -> Result<(), Self::Error> {
        socket.close();
        Ok(())
    }
}

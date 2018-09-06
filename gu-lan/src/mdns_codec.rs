use tokio_codec::{Decoder, Encoder};
use errors::{Error, ErrorKind, Result};
use bytes::BytesMut;
use service::Service;

use dns_parser::{Builder, Packet, QueryClass, QueryType};
use dns_parser::rdata::RData::SRV;
use service::ServiceInstance;
use std::collections::HashMap;
use std::net::IpAddr;
use dns_parser::rdata::RData::TXT;
use std::str::from_utf8;
use dns_parser::rdata::RData::A;
use dns_parser::rdata::a::Record;

pub type ParsedPacket = (u16, Vec<(String, ServiceInstance)>);

#[derive(Debug)]
pub(crate) struct MdnsCodec;

impl Decoder for MdnsCodec {
    type Item = ParsedPacket;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<ParsedPacket>> {
        let packet = Packet::parse(src.as_ref())?;
        info!("Received packet: {:?}", packet);

        let id = packet.header.id;
        let mut services : Vec<(String, ServiceInstance)> = Vec::new();

        // (service, host) -> ports
        let mut srv_map : HashMap<(String, String), Vec<u16>> = HashMap::new();
        // service -> description
        let mut txt_map : HashMap<String, Vec<String>> = HashMap::new();
        // host -> IPv4
        let mut a_map   : HashMap<String, Vec<IpAddr>> = HashMap::new();

        for answer in packet.answers {
            match answer.data {
                SRV(data) => {
                    let key = (answer.name.to_string(), data.target.clone().to_string());
                    srv_map
                        .entry(key)
                        .or_insert_with(|| Vec::new())
                        .push(data.port);
                },
                TXT(data) => {
                    txt_map.insert(answer.name.to_string(), data.iter()
                        .map(|x| from_utf8(x).unwrap_or_default().to_string())
                        .collect::<Vec<_>>());
                },
                A(data) => {
                    let Record(arr) = data;
                    a_map.entry(answer.name.to_string())
                        .or_default()
                        .push(arr.into());
                }
                _ => ()
            }
        }

        srv_map.into_iter().for_each(|a| {
            let pair = a.0;
            let name = pair.0;
            let host = pair.1;
            let ports = a.1;

            let addrs = a_map.get(&host).map(|a| a.clone()).unwrap_or(Vec::new());
            let txt = txt_map.get(&name).map(|a| a.clone()).unwrap_or(Vec::new());

            services.push((name, ServiceInstance { host, txt, addrs, ports }))
        });

        Ok(Some((id, services)))
    }
}

impl Encoder for MdnsCodec {
    type Item = (Service, u16);
    type Error = Error;

    fn encode(&mut self, item: (Service, u16), dst: &mut BytesMut) -> Result<()> {
        let mut builder = Builder::new_query(item.1, false);
        builder.add_question(item.0.to_string().as_ref(), true, QueryType::SRV, QueryClass::IN);
        builder.add_question(item.0.to_string().as_ref(), true, QueryType::TXT, QueryClass::IN);
        let packet = builder
            .build()
            .map_err(ErrorKind::DnsPacketBuildError)?;
        info!("Encoded packet to send: {:?}", packet);

        dst.extend_from_slice(packet.as_ref());
        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use mdns_codec::MdnsCodec;
    use tokio_codec::{Decoder, Encoder};
    use bytes::BytesMut;
    use service::Service;

    #[test]
    fn decode_packet() {
        let a = b"\0\0\x80\0\0\0\0\x03\0\0\0\0\x0bgu-provider\x05_http\x04_tcp\x05local\0\0!\0\x01\0\0\0<\0\x18\0\0\0\0\0P\nimapp-1023\x05local\0\nimapp-1023\x05local\0\0\x01\0\x01\0\0\0<\0\x04\n\x1e\x08\xce\nimapp-1023\x05local\0\0\x01\0\x01\0\0\0<\0\x04\xac\x11\0\x01";

        let mut bytes = BytesMut::new();
        bytes.extend_from_slice(a);

        let packet = MdnsCodec{}.decode(&mut bytes);

        assert!(packet.is_ok());
        let packet = packet.unwrap();

        assert!(packet.is_some());
        let packet = packet.unwrap();

        assert_eq!(packet.id, 0);
        assert!(packet.list.get(0).is_some());
        assert_eq!(packet.list.get(0).unwrap().name, "gu-provider._http._tcp.local");
    }

    #[test]
    fn encode_packet() {
        let service = Service::new("gu-provider", "_http._tcp");
        let mut bytes = BytesMut::new();
        let packet = MdnsCodec{}.encode((service, 124), &mut bytes);

        let a = b"\0|\0\0\0\x01\0\0\0\0\0\0\x0bgu-provider\x05_http\x04_tcp\x05local\0\0!\x80\x01";
        let mut bytes2 = BytesMut::new();
        bytes2.extend_from_slice(a);

        assert!(packet.is_ok());
        assert_eq!(bytes,  bytes2);
    }
}
use bytes::BytesMut;
use errors::{Error, ErrorKind, Result};
use tokio_codec::{Decoder, Encoder};

use dns_parser::rdata::a::Record;
use dns_parser::rdata::RData::A;
use dns_parser::rdata::RData::SRV;
use dns_parser::rdata::RData::TXT;
use dns_parser::ResourceRecord;
use dns_parser::{Builder, Packet, QueryClass, QueryType};
use service::ServiceInstance;
use service::ServicesDescription;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::from_utf8;

pub type ParsedPacket = (u16, Vec<(String, ServiceInstance)>);

#[derive(Debug)]
pub(crate) struct MdnsCodec;

fn parse_answer(answer: ResourceRecord, parse_maps: &mut ParseMaps) {
    match answer.data {
        SRV(data) => {
            let key = (answer.name.to_string(), data.target.clone().to_string());
            parse_maps.srv.entry(key).or_default().push(data.port);
        }
        TXT(data) => {
            parse_maps.txt.insert(
                answer.name.to_string(),
                data.iter()
                    .map(|x| from_utf8(x).unwrap_or_default().to_string())
                    .collect::<Vec<_>>(),
            );
        }
        A(data) => {
            let Record(arr) = data;
            parse_maps
                .a
                .entry(answer.name.to_string())
                .or_default()
                .push(arr.into());
        }
        _ => (),
    }
}

fn build_response(parse_maps: ParseMaps, services: &mut Vec<(String, ServiceInstance)>) {
    let srv = parse_maps.srv;
    let a = parse_maps.a;
    let txt = parse_maps.txt;
    srv.into_iter().for_each(move |e| {
        let pair = e.0;
        let name = pair.0;
        let host = pair.1;
        let ports = e.1;

        let addrs = a.get(&host).map(|a| a.clone()).unwrap_or(Vec::new());
        let txt = txt.get(&name).map(|a| a.clone()).unwrap_or(Vec::new());

        services.push((
            name.clone(),
            ServiceInstance {
                name,
                host,
                txt,
                addrs,
                ports,
            },
        ))
    });
}

#[derive(Default)]
struct ParseMaps {
    // (service, host) -> ports
    pub srv: HashMap<(String, String), Vec<u16>>,
    // service -> description
    pub txt: HashMap<String, Vec<String>>,
    // host -> IPv4
    pub a: HashMap<String, Vec<IpAddr>>,
}

impl Decoder for MdnsCodec {
    type Item = ParsedPacket;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<ParsedPacket>> {
        let packet = Packet::parse(src.as_ref())?;
        info!("Received packet: {:?}", packet);

        let id = packet.header.id;
        let mut parse_maps = ParseMaps::default();

        for answer in packet.answers {
            parse_answer(answer, &mut parse_maps);
        }

        for answer in packet.additional {
            parse_answer(answer, &mut parse_maps);
        }

        let mut services: Vec<(String, ServiceInstance)> = Vec::new();
        build_response(parse_maps, &mut services);
        Ok(Some((id, services)))
    }
}

impl Encoder for MdnsCodec {
    type Item = (ServicesDescription, u16);
    type Error = Error;

    fn encode(&mut self, item: (ServicesDescription, u16), dst: &mut BytesMut) -> Result<()> {
        let mut builder = Builder::new_query(item.1, false);
        for service in item.0.services().iter() {
            builder.add_question(
                service.to_string().as_ref(),
                true,
                QueryType::SRV,
                QueryClass::IN,
            );
            builder.add_question(
                service.to_string().as_ref(),
                true,
                QueryType::TXT,
                QueryClass::IN,
            );
        }
        let packet = builder.build().map_err(ErrorKind::DnsPacketBuildError)?;
        info!("Encoded packet to send: {:?}", packet);

        dst.extend_from_slice(packet.as_ref());
        Ok(())
    }
}

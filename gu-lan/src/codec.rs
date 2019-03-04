use bytes::BytesMut;
use errors::{Error, ErrorKind, Result};
use tokio_codec::{Decoder, Encoder};

use dns_parser::{
    rdata::{
        a::Record,
        RData::{A, SRV, TXT},
    },
    Builder, Packet, QueryClass, QueryType, Question, ResourceRecord,
};
use service::{ServiceInstance, ServicesDescription};
use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    str::from_utf8,
};

#[derive(Clone, Debug)]
pub struct ParsedPacket {
    pub id: u16,
    pub instances: Vec<ServiceInstance>,
    pub questions: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct MdnsCodec(pub bool);

fn parse_question(question: Question, parse_sets: &mut QuestionParseSets) {
    if question.prefer_unicast || question.qclass != QueryClass::IN {
        return;
    }
    let name = question.qname.to_string();

    match question.qtype {
        QueryType::SRV => {
            parse_sets.srv.insert(name);
        }
        QueryType::TXT => {
            parse_sets.txt.insert(name);
        }
        _ => (),
    };
}

fn combine_questions(mut parse_sets: QuestionParseSets, questions: &mut Vec<String>) {
    for name in parse_sets.srv {
        if parse_sets.txt.remove(&name) {
            questions.push(name)
        }
    }
}

fn parse_answer(answer: ResourceRecord, parse_maps: &mut ResponseParseMaps) {
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

fn combine_answers(parse_maps: ResponseParseMaps, services: &mut Vec<ServiceInstance>) {
    let srv = parse_maps.srv;
    let a = parse_maps.a;
    let txt = parse_maps.txt;
    srv.into_iter().for_each(move |e| {
        let pair = e.0;
        let name = pair.0;
        let host = pair.1;
        let ports = e.1;

        let addrs_v4 = a.get(&host).map(|a| a.clone()).unwrap_or(Vec::new());
        let txt = txt.get(&name).map(|a| a.clone()).unwrap_or(Vec::new());

        services.push(ServiceInstance {
            name,
            host,
            txt,
            addrs_v4,
            ports,
        })
    });
}

#[derive(Default)]
struct ResponseParseMaps {
    // (service, host) -> ports
    pub srv: HashMap<(String, String), Vec<u16>>,
    // service -> description
    pub txt: HashMap<String, Vec<String>>,
    // host -> IPv4
    pub a: HashMap<String, Vec<Ipv4Addr>>,
}

#[derive(Default, Debug)]
struct QuestionParseSets {
    pub srv: HashSet<String>,
    pub txt: HashSet<String>,
}

impl Decoder for MdnsCodec {
    type Item = ParsedPacket;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<ParsedPacket>> {
        let packet = Packet::parse(src.as_ref())?;
        let id = packet.header.id;

        let mut parse_maps = ResponseParseMaps::default();
        let mut parse_sets = QuestionParseSets::default();

        for question in packet.questions {
            parse_question(question, &mut parse_sets)
        }

        for answer in packet.answers {
            parse_answer(answer, &mut parse_maps);
        }

        for answer in packet.additional {
            parse_answer(answer, &mut parse_maps);
        }

        let mut services: Vec<ServiceInstance> = Vec::new();
        let mut questions: Vec<String> = Vec::new();

        combine_answers(parse_maps, &mut services);
        combine_questions(parse_sets, &mut questions);

        debug!("Decoded mDNS packet {:?}", services);

        Ok(Some(ParsedPacket {
            id,
            instances: services,
            questions,
        }))
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
                self.0,
                QueryType::PTR,
                QueryClass::IN,
            );
        }
        let packet = builder.build().map_err(ErrorKind::DnsPacketBuildError)?;
        debug!("Encoded packet to send: {:?}", packet);

        dst.extend_from_slice(packet.as_ref());
        Ok(())
    }
}

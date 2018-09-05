use tokio_codec::{Decoder, Encoder};
use errors::{Error, ErrorKind, Result};
use bytes::BytesMut;
use service::Service;

use dns_parser::{Builder, Packet, QueryClass, QueryType};
use dns_parser::rdata::RData::SRV;

#[derive(Debug)]
pub(crate) struct ServiceAnswer {
    pub port: u16,
    pub name: String,
}

#[derive(Debug)]
pub(crate) struct ServiceAnswers {
    pub id: u16,
    pub list: Vec<ServiceAnswer>,
}

#[derive(Debug)]
pub(crate) struct MdnsCodec;

impl Decoder for MdnsCodec {
    type Item = ServiceAnswers;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<ServiceAnswers>> {
        let packet = Packet::parse(src.as_ref())?;
        info!("Received packet: {:?}", packet);

        let id = packet.header.id;
        let mut list = Vec::new();
        for answer in packet.answers {
            if let SRV(data) = answer.data {
                list.push(
                    ServiceAnswer {
                        port: data.port,
                        name: answer.name.to_string(),
                    });
            }
        }

        Ok(Some(ServiceAnswers {
            id,
            list,
        }))
    }
}

impl Encoder for MdnsCodec {
    type Item = (Service, u16);
    type Error = Error;

    fn encode(&mut self, item: (Service, u16), dst: &mut BytesMut) -> Result<()> {
        let mut builder = Builder::new_query(item.1, false);
        builder.add_question(item.0.to_string().as_ref(), true, QueryType::SRV, QueryClass::IN);
        let packet = builder
            .build()
            .map_err(ErrorKind::DnsPacketBuildError)?;
        info!("Encoded packet to send: {:?}", packet);

        dst.extend_from_slice(packet.as_ref());
        Ok(())
    }
}

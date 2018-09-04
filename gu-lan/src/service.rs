use std::borrow::Cow;
use actix::Message;
use dns_parser::Header;
use errors::Result;

/// Struct describing single service in .local domain's network
///
/// Service Instance Name = <Instance> . <Service> . <Domain>
#[derive(Debug)]
pub struct Service {
    /// Instance name; eg. "gu-provider"
    instance: Cow<'static, str>,
    /// Service type; eg. "_http._tcp"
    service: Cow<'static, str>,
}

impl Service {
    pub fn new<A, B>(instance: A, service: B) -> Self
        where
            A: Into<Cow<'static, str>>,
            B: Into<Cow<'static, str>>,
    {
        Service {
            instance: instance.into(),
            service: service.into(),
        }
    }

    pub(crate) fn to_string(&self) -> String {
        format!("{}.local", self.service)
    }
}

impl Message for Service {
    type Result = Result<ServicesList>;
}

#[derive(Debug)]
pub struct ServiceInstance {
    pub data: Header,
}

pub type ServicesList = Vec<ServiceInstance>;
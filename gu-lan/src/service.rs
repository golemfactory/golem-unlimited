use actix::Message;
use errors::Result;
use std::borrow::Cow;
use std::collections::HashSet;
use std::net::IpAddr;

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
        format!("{}.{}.local", self.instance, self.service)
    }
}

impl Message for Service {
    type Result = Result<HashSet<ServiceInstance>>;
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ServiceInstance {
    pub host: String,
    pub txt: Vec<String>,
    pub addrs: Vec<IpAddr>,
    pub ports: Vec<u16>,
}

#[derive(Debug)]
pub(crate) struct Services {
    name: String,
    set: HashSet<ServiceInstance>,
}

impl Services {
    pub(crate) fn new(name: String) -> Self {
        Services {
            name,
            set: HashSet::new(),
        }
    }

    pub(crate) fn add_instance(&mut self, name: String, instance: ServiceInstance) {
        if name == self.name {
            self.set.insert(instance);
        }
    }

    pub(crate) fn set(self) -> HashSet<ServiceInstance> {
        self.set
    }
}

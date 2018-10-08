use actix::prelude::*;
use errors::Result;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
};

/// Struct describing single service in .local domain's network
///
/// Service Instance Name = <Instance> . <Service> . <Domain>
#[derive(Debug, Clone)]
pub struct ServiceDescription {
    /// Instance name; eg. "gu-provider"
    instance: Cow<'static, str>,
    /// Service type; eg. "_http._tcp"
    service: Cow<'static, str>,
}

impl ServiceDescription {
    pub fn new<A, B>(instance: A, service: B) -> Self
    where
        A: Into<Cow<'static, str>>,
        B: Into<Cow<'static, str>>,
    {
        ServiceDescription {
            instance: instance.into(),
            service: service.into(),
        }
    }

    pub(crate) fn to_string(&self) -> String {
        format!("{}.{}.local", self.instance, self.service)
    }
}

impl Message for ServiceDescription {
    type Result = Result<HashSet<ServiceInstance>>;
}

#[derive(Debug, Clone)]
pub struct ServicesDescription {
    services: Vec<ServiceDescription>,
}

impl ServicesDescription {
    pub fn new(services: Vec<ServiceDescription>) -> Self {
        ServicesDescription { services }
    }

    pub fn single<A, B>(instance: A, service: B) -> Self
    where
        A: Into<Cow<'static, str>>,
        B: Into<Cow<'static, str>>,
    {
        Self::new(vec![ServiceDescription::new(instance, service)])
    }

    pub(crate) fn services(&self) -> &Vec<ServiceDescription> {
        &self.services
    }

    pub(crate) fn to_services(&self) -> Services {
        let mut services = Services::default();
        for i in self.services.clone() {
            services.add_service(i.to_string())
        }
        services
    }
}

impl Message for ServicesDescription {
    type Result = Result<HashSet<ServiceInstance>>;
}

/// Contains information about single service in a network
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ServiceInstance {
    pub name: String,
    pub host: String,
    pub txt: Vec<String>,
    pub addrs_v4: Vec<Ipv4Addr>,
    pub ports: Vec<u16>,
}

#[derive(Debug, Serialize, Default)]
pub struct Services {
    map: HashMap<String, HashSet<ServiceInstance>>,
}

impl<'a> From<&'a ServicesDescription> for Services {
    fn from(s: &'a ServicesDescription) -> Self {
        let mut res = Services::default();
        for service in s.services() {
            res.add_service(service.to_string());
        }

        res
    }
}

impl Services {
    pub(crate) fn add_service(&mut self, s: String) {
        self.map.insert(s, HashSet::new());
    }

    pub(crate) fn add_instance(&mut self, instance: ServiceInstance) {
        self.map
            .get_mut::<str>(instance.name.as_ref())
            .and_then(|map| Some(map.insert(instance)));
    }

    pub(crate) fn collect(self) -> HashSet<ServiceInstance> {
        let mut set: HashSet<ServiceInstance> = HashSet::new();
        for i in self.map {
            set.extend(i.1)
        }
        set
    }
}

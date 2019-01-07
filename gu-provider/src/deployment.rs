use gu_model::envman::Error;
use gu_net::rpc::peer::PeerSessionInfo;
use id::generate_new_id;
use status;
use std::collections::HashMap;

pub trait IntoDeployInfo {
    fn convert(&self, id: &String) -> PeerSessionInfo;
}

pub trait Destroy {
    fn destroy(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

pub trait GetStatus {
    fn status(&self) -> status::EnvStatus;
}

impl<T: IntoDeployInfo> GetStatus for T {
    fn status(&self) -> status::EnvStatus {
        let info = self.convert(&Default::default());
        debug!("session status = {:?}", info.status);

        match info.processes.is_empty() {
            true => status::EnvStatus::Ready,
            false => status::EnvStatus::Working,
        }
    }
}

pub struct DeployManager<DeployInfo: IntoDeployInfo + Destroy> {
    deploys: HashMap<String, DeployInfo>,
}

impl<T: IntoDeployInfo + Destroy> Default for DeployManager<T> {
    fn default() -> Self {
        Self {
            deploys: HashMap::new(),
        }
    }
}

impl<T: IntoDeployInfo + Destroy + GetStatus> DeployManager<T> {
    pub fn generate_session_id(&self) -> String {
        generate_new_id(&self.deploys)
    }

    pub fn insert_deploy(&mut self, id: String, deploy: T) {
        self.deploys.insert(id, deploy);
    }

    pub fn contains_deploy(&self, key: &String) -> bool {
        self.deploys.contains_key(key)
    }

    pub fn deploy_mut(&mut self, deploy_id: &String) -> Result<&mut T, Error> {
        match self.deploys.get_mut(deploy_id) {
            Some(deploy) => Ok(deploy),
            None => Err(Error::NoSuchSession(deploy_id.clone())),
        }
    }

    pub fn destroy_deploy(&mut self, session_id: &String) -> Result<(), Error> {
        self.deploys
            .remove(session_id)
            .ok_or(Error::NoSuchSession(session_id.clone()))
            .and_then(|mut s| s.destroy())
    }

    pub fn deploys_info(&self) -> Vec<PeerSessionInfo> {
        self.deploys
            .iter()
            .map(|(id, session)| session.convert(id))
            .collect()
    }

    pub fn values_mut<'a>(&'a mut self) -> impl Iterator<Item = &mut T> + 'a {
        self.deploys.values_mut().into_iter()
    }

    pub fn status(&self) -> status::EnvStatus {
        match self
            .deploys
            .iter()
            .all(|(_, info)| info.status() == status::EnvStatus::Ready)
        {
            true => status::EnvStatus::Ready,
            false => status::EnvStatus::Working,
        }
    }
}

impl<T: IntoDeployInfo + Destroy> Drop for DeployManager<T> {
    fn drop(&mut self) {
        let _: Vec<Result<(), Error>> = self.deploys.values_mut().map(Destroy::destroy).collect();
        println!("HdMan stopped");
    }
}

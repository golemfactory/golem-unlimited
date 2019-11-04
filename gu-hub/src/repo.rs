use actix_web::error::{ErrorBadRequest, ErrorInternalServerError};
use actix_web::fs::NamedFile;
use actix_web::{App, HttpMessage, HttpRequest, HttpResponse};
use futures::prelude::*;
use gu_base::{Decorator, Module};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, RwLock};

use gu_persist::config::ConfigModule;
use tempfile::NamedTempFile;

struct RepoModule {
    // repo, repo_cache
    paths: Mutex<Option<(PathBuf, PathBuf)>>,
}

impl Module for RepoModule {
    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let config_module: &ConfigModule = decorator.extract().unwrap();

        let repo = config_module.cache_dir().join("repo");
        let repo_cache = config_module.cache_dir().join("repo-temp");

        let mut g = self.paths.lock().unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&repo_cache).unwrap();

        *g = Some((repo, repo_cache))
    }

    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        let (repo, repo_cache) = self.paths.lock().unwrap().clone().unwrap();

        let repo_temp: Rc<Path> = repo_cache.into();

        let cache_path: Rc<Path> = repo.into();
        let cache_path_get = cache_path.clone();
        std::fs::create_dir_all(&cache_path).unwrap();

        app
                .resource("/repo", |r| {
                r.post().with_async(move |r: HttpRequest<S>| {
                    let cache_path = cache_path.clone();
                    let lob_file = Rc::new(RwLock::new(gu_actix::async_try!(NamedTempFile::new_in(repo_temp.as_ref()))));
                    let sha1 = Rc::new(RwLock::new(sha1::Sha1::new()));

                    let lob_file_f = lob_file.clone();
                    let sha1_f = sha1.clone();

                    gu_actix::async_result! {
                    r.payload()
                        .map_err(|e| ErrorBadRequest(format!("Couldn't get request body: {:?}", e)))
                        .for_each(move |chunk| {
                            lob_file.write().unwrap().write_all(chunk.as_ref())?;
                            sha1.write().unwrap().update(chunk.as_ref());
                            Ok(())
                        })
                        .and_then(move |()| {
                            let hash = sha1_f.write().unwrap().hexdigest();

                            Rc::try_unwrap(lob_file_f).unwrap()
                                .into_inner()
                                .unwrap()
                                .persist(cache_path.join(&hash))
                                .map_err(|e| ErrorInternalServerError(format!("Couldn't save image: {:?}", e)))?;

                            Ok(HttpResponse::Created()
                                .header("Location", format!("/repo/{}", hash))
                                .json(hash))
                        })
                }
                })
            })
            .resource("/repo/{hash}", move |r| {
                let cache_path = cache_path_get.clone();
                r.get().with(move |p: actix_web::Path<(String, )>| -> Result<NamedFile, actix_web::Error>{
                    let hexhash = p.0.as_str();
                    let file_path = cache_path.join(hexhash);
                    Ok(NamedFile::open(file_path)?)
                })
            })
    }
}

pub fn module() -> impl Module {
    RepoModule {
        paths: Mutex::new(None),
    }
}

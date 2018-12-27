use super::Module;

struct EmptyModule;

impl Module for EmptyModule {}

pub fn module() -> impl Module {
    EmptyModule
}

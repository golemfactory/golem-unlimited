

#[derive(Default, Serialize, Deserialize)]
struct CreateOptions {
    pub volumes : Vec<VolumeDef>,
    pub cmd : Option<Vec<String>>,


}

#[derive(Serialize, Deserialize)]
pub enum VolumeDef {
    BindRw {
        src : String,
        target : String,
    }

}


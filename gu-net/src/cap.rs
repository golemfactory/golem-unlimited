

type cow_str = Cow<'static,str>;

pub enum SpecAtom {
    Int(i64),
    Version(i16, i16, i16),
    Str(cow_str),
    StrVec(Vec<cow_str>)
}

pub struct Spec {
    inner : Vec<(cow_str, SpecAtom)>
}

impl Spec {

}
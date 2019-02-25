use update_macro::Update;
use update_trait::UpdateTrait;

#[derive(Update, Debug, Default)]
struct A {
    b: B,
}

#[derive(Update, Debug, Default)]
struct B {
    c2: C,
    c: C,
}

#[derive(Update, Debug, Default)]
struct C {
    arg: bool,
    opt: Option<u8>,
}

enum E {
    A(A),
}


#[test]
fn works() {
    let mut a = A::default();
    a.b.c.opt = Some(1);
    println!(
        "{:?}",
        a.update(
            ["b", "c", "arg"].to_vec().iter().map(|x| x.to_string()),
            "true".to_string()
        )
    );

    println!(
        "{:?}",
        a.clear(["b", "c", "opt"].to_vec().iter().map(|x| x.to_string()))
    );

    println!("{:?}", a);

    let e = E::A(A::default());
}

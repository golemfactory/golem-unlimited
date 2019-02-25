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

#[derive(Update, Debug)]
enum E {
    AA(A),
    BB(B),
}

#[test]
fn works() {
    macro_rules! iter_of_strings {
        ($($x:expr),*) => (vec![$($x.to_string()),*].into_iter());
    }

    let mut a = A::default();
    a.b.c.opt = Some(1);
    println!(
        "{:?}",
        a.update(iter_of_strings!["b", "c", "arg"], "true".to_string())
    );
    assert_eq!(a.b.c.arg, true);

    println!("{:?}", a.clear(iter_of_strings!["b", "c", "opt"]));
    assert_eq!(a.b.c.opt, None);

    a.b.c.opt = Some(1);
    let mut e = E::AA(a);

    println!(
        "{:?}",
        e.clear(iter_of_strings!("AA", "b", "c", "opt"))
    );

    if let E::AA(ref x) = e {
        assert_eq!(x.b.c.opt, None);
    } else {
        panic!("Wrong option in enum")
    }

    println!(
        "{:?}",
        e.update(iter_of_strings!("AA", "b", "c", "arg"), "false".into())
    );

    if let E::AA(ref x) = e {
        assert_eq!(x.b.c.arg, false);
    } else {
        panic!("Wrong option in enum")
    }
}

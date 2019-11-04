use update_macro::Update;
use update_trait::UpdateTrait;

#[derive(Update, Debug, Default, PartialEq)]
struct A {
    b: B,
}

#[derive(Update, Debug, Default, PartialEq)]
struct B {
    c2: C,
    c: C,
}

#[derive(Update, Debug, Default, PartialEq)]
struct C {
    arg: bool,
    opt: Option<u8>,
}

#[derive(Update, Debug, PartialEq)]
enum E {
    Single(A),
    // checks only if compilation for such case works
    #[allow(unused)]
    Double(B, B),
    Empty,
    Click {
        x: i64,
        opt: Option<bool>,
    },
}

#[test]
fn works() {
    macro_rules! iter_of_strings {
        ($($x:expr),*) => (vec![$($x.to_string()),*].into_iter());
    }

    let mut a = A::default();
    a.b.c.opt = Some(1);
    assert!(a
        .set(iter_of_strings!["b", "c", "arg"], "true".to_string())
        .is_ok());
    assert_eq!(a.b.c.arg, true);

    assert!(a.remove(iter_of_strings!["b", "c", "opt"]).is_ok());
    assert_eq!(a.b.c.opt, None);

    a.b.c.opt = Some(1);
    let mut e = E::Single(a);

    assert!(e
        .remove(iter_of_strings!("Single", "b", "c", "opt"))
        .is_ok());

    if let E::Single(ref x) = e {
        assert_eq!(x.b.c.opt, None);
    } else {
        panic!("Wrong option in enum")
    }

    let _ = e.set(iter_of_strings!("Single", "b", "c", "arg"), "false".into());
    if let E::Single(ref x) = e {
        assert_eq!(x.b.c.arg, false);
    } else {
        panic!("Wrong option in enum")
    }

    let mut x = E::Click {
        x: 0,
        opt: Some(true),
    };

    let _ = x.remove(iter_of_strings!("Click", "opt"));
    let _ = x.set(iter_of_strings!("Click", "x"), "123".into());

    if let E::Click { x, opt } = x {
        assert_eq!(opt, None);
        assert_eq!(x, 123);
    } else {
        panic!("Wrong option in enum")
    }

    let mut x = E::Click {
        x: 0,
        opt: Some(true),
    };
    let _ = x.set(iter_of_strings!("Empty"), "".into());
    assert_eq!(x, E::Empty);
}

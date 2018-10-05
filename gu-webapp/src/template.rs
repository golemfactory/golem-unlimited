use regex::RegexSet;

struct Parser<'a> {
    buf : &'a [u8]
}

enum Token<'a> {
    Data(&'a [u8]),
    Script(&'a str),
    Link(&'a str)
}

enum State {
    Init,
    HL,
    HLi,
    HLim,
    HLimg,
    HLimgw,
    HLimgwx,
    HLimgws,
    HLimgwsr,
    HLimgwsrc,
    HLimgwsrce,
    HLimgwsrceq(usize, u8),

    HLx,

}

impl<'a> Iterator for Parser<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        let len = self.buf.len();
        let mut s = State::Init;

        while pos < len {
            let b = self.buf[pos];

            let next_state = match (s, b) {
                (State::Init, b'<') => State::HL,
                (State::Init, _) => State::Init,

                (State::HL, b'i') => State::HLi,
                (State::HL, b'>') => State::Init,
                (State::HL, _) => State::HLx,

                (State::HLi, b'm') => State::HLim,
                (State::HLi, b'>') => State::Init,
                (State::HLi, _) => State::HLx,

                (State::HLim, b'g') => State::HLimg,
                (State::HLim, b'>') => State::Init,
                (State::HLim, _) => State::HLx,

                (State::HLimg, b' ') => State::HLimgw,
                (State::HLimg, b'>') => State::Init,
                (State::HLimg, _) => State::HLx,

                (State::HLimgw, b's') => State::HLimgws,
                (State::HLimg, b'>') => State::Init,
                (State::HLimg, _) => State::HLx,


            };
            pos += 1;
            s = next_state;
        }

    }
}

#[cfg(test)]
mod test {
    use regex::*;
    use super::*;

    #[test]
    fn test_x() {
        let m : Vec<_> = prepare().matches("aaa aaa aaa <img src=\"ala.png\">").into_iter().collect();

        println!("{:?}", m)
    }

}
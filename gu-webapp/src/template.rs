pub struct Parser<'a> {
    buf : &'a [u8],
    state : State<'a>,
    pos : usize,
}

impl<'a> From<&'a str> for Parser<'a> {
    fn from(s: &'a str) -> Self {
        Parser {
            buf: s.as_bytes(),
            state: State::Init(0),
            pos: 0
        }
    }
}


pub enum Token<'a> {
    Start(&'a [u8]),
    Attr(&'a [u8], &'a [u8]),
    End(&'a [u8]),
    Data(&'a [u8]),
}

#[derive(Clone)]
enum State<'a> {
    Init(usize),
    LT,    // '<' [*]
    LTx(usize),   // '<' 'i' 'm' [*] 'g' ' ' 's' 'r' 'c' '=' '"' ...
    LTxw,  // '<' 'i' 'm' 'g' ' '[*] 's' 'r' 'c' '=' '"' ...
    AN(usize),    // '<' 'i' 'm' 'g' ' ' 's' 'r' [*] 'c' '=' '"' ...
    ANe(&'a [u8]),   // '<' 'i' 'm' 'g' ' ' 's' 'r' 'c' '=' [*] '"' ...
    ANv(&'a [u8], u8, usize),   // '<' 'i' 'm' 'g' ' ' 's' 'r' 'c' '=' '"' [*] ...

    LTs,   // '<' '/' [*]
    LTsx(usize),  // '<' '/' 'i' 'm' [*] 'g' ' ' '>'
    LTsxw, // '<' '/' 'i' 'm' [*] 'g' ' ' [*] '>'

    LTe,
}

impl<'a> Iterator for Parser<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        let len = self.buf.len();
        let mut s = self.state.clone();

        while self.pos < len {
            let pos = self.pos;
            let b = self.buf[pos];

            let (next_state, token) = match (s, b) {
                (State::Init(spos), b'<') => (State::LT, Some(Token::Data(&self.buf[spos..pos]))),
                (State::Init(spos), _) => (State::Init(spos), None),

                (State::LT, b'>') => (State::Init(pos), None),
                (State::LT, b' ') => (State::LTe, None),
                (State::LT, b'/') => (State::LTs, None),
                (State::LT, _ch) => (State::LTx(pos), None),

                (State::LTx(spos), b'>') => (State::Init(pos+1), Some(Token::Start(&self.buf[spos..pos]))),
                (State::LTx(spos), b' ') => (State::LTxw, Some(Token::Start(&self.buf[spos..pos]))),
                (State::LTx(pos), _ch) => (State::LTx(pos), None),


                (State::LTxw, b'>') => (State::Init(pos), None),
                (State::LTxw, b' ') => (State::LTxw, None),
                (State::LTxw, ch) => (State::AN(pos), None),

                (State::AN(spos), b'>') => (State::Init(pos), Some(Token::Attr(&self.buf[spos..pos], &[]))),
                (State::AN(spos), b' ') => (State::LTxw, Some(Token::Attr(&self.buf[spos..pos], &[]))),
                (State::AN(spos), b'=') => (State::ANe(&self.buf[spos..pos]), None),
                (State::AN(spos), _ch) => (State::AN(spos), None),

                (State::ANe(k), b' ') => (State::ANe(k), None),
                (State::ANe(k), b'\"') => (State::ANv(k, b'\"', pos+1), None),
                (State::ANe(k), b'\'') => (State::ANv(k, b'\'', pos+1), None),
                (State::ANe(k), _) => (State::LTe, None),

                (State::ANv(k, ech1, spos), ech2) if ech1 == ech2 => (State::LTsxw, Some(Token::Attr(k, &self.buf[spos..pos]))),
                (State::ANv(k, ech, spos), _ch) => (State::ANv(k, ech, spos), None),

                (State::LTs, b'>') => (State::Init(pos+1), None),
                (State::LTs, b' ') => (State::LTe, None),
                (State::LTs, ch) => (State::LTsx(pos), None),

                (State::LTsx(spos), b'>') => (State::Init(pos+1), Some(Token::End(&self.buf[spos..pos]))),
                (State::LTsx(spos), b' ') => (State::LTsxw, Some(Token::End(&self.buf[spos..pos]))),
                (State::LTsx(spos), _ch) => (State::LTsx(spos), None),


                (State::LTsxw, b'>') => (State::Init(pos+1), None),
                (State::LTsxw, b' ') => (State::LTsxw, None),
                (State::LTsxw, _) => (State::LTe, None),

                (State::LTe, b'>') => (State::Init(pos+1), None),
                (State::LTe, _) => (State::LTe, None),
            };
            self.pos += 1;
            s = next_state;
            if let Some(next_token) = token {
                self.state = s;
                return Some(next_token)
            }
        }
        None
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_x() {
        use std::str::from_utf8;
        let m : Parser = r#"
           <div class="row">
                <div class="panel panel-default">
                    <div class="panel-heading">
                        Plugins
                    </div>
                    <div class="panel-body">
                        <ul class="nav nav-pills nav-stacked">
                            <li ng-repeat="tab in pluginTabs" ng-class="{active: tab == activeTab}">
                                <a href="" ng-click="openTab(tab)"><img ng-src="{{tab.icon}}" width="30" height="30" ng-if="!!tab.icon">
                                    {{tab.name}}
                                </a>
                            </li>
                        </ul>
                    </div>
                </div>
            </div>

        </div>
        <div class="col-md-10">
            <div class="row" ng-include="activeTab.page">
            ala {{activeTab | json }}
                / {{tab.name }}
            </div>
        </div>
    </div>
</div>
<script src="ui-bootstrap-tpls-2.5.0.min.js"></script>
<script src="app.js"></script>
</body>
        "#.into();
        for token in m {
            match token {
                Token::Start(tag_name) => println!("start '{}'", from_utf8(tag_name).unwrap()),
                Token::End(tag_name) =>   println!("end   '{}'", from_utf8(tag_name).unwrap()),
                Token::Attr(key, val) =>  println!("attr  -- '{}' = '{}'", from_utf8(key).unwrap(), from_utf8(val).unwrap()),
                Token::Data(d) =>         println!("data  '{}'", from_utf8(d).unwrap())
            }
        }

    }

}
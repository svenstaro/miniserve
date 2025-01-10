/// "percent-encode sets" as defined by WHATWG specs:
/// https://url.spec.whatwg.org/#percent-encoded-bytes
pub mod percent_encode_sets {
    use percent_encoding::{AsciiSet, CONTROLS};
    pub const QUERY: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');
    pub const PATH: &AsciiSet = &QUERY.add(b'?').add(b'`').add(b'{').add(b'}');
    pub const USERINFO: &AsciiSet = &PATH
        .add(b'/')
        .add(b':')
        .add(b';')
        .add(b'=')
        .add(b'@')
        .add(b'[')
        .add(b'\\')
        .add(b']')
        .add(b'^')
        .add(b'|');
    pub const COMPONENT: &AsciiSet = &USERINFO.add(b'$').add(b'%').add(b'&').add(b'+').add(b',');
}

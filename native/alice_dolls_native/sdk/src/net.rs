//! What the server half tells each player's client, and what a client tells the server, as mod
//! messages (`net_send_to_player`, `net_send_to_server`). In single player both halves share the
//! process and could read each other's state; on a server, or as a LAN guest, they can't, so
//! everything a client draws from the dolls' goals comes in these.

/// Each tick, to each player: what to draw of the dolls in their level near them.
pub const FX: &str = "alice_dolls_native:fx";
/// A doll inventory screen opened, changed or closed: its layout, for the slot outlines and scrolling.
pub const MENU: &str = "alice_dolls_native:menu";
/// One of the player's Aya dolls took a photo: the screenshot is theirs to take.
pub const PHOTO: &str = "alice_dolls_native:photo";
/// From a client: the mouse wheel turned over a long doll inventory, rows down (negative up).
pub const ROCK_SWITCH: &str = "alice_dolls_native:rock_switch";
pub const SCROLL: &str = "alice_dolls_native:scroll";

/// The most shots and beams one message carries: a message holds 30 000 bytes.
pub const MAX_SHOTS: usize = 400;
pub const MAX_BEAMS: usize = 64;
const MAX_IDS: usize = 512;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fx {
    pub shots: Vec<(u8, [f64; 3], [f64; 3])>,
    pub beams: Vec<([f64; 3], [f64; 3])>,
    pub swings: Vec<(u32, f32)>,
    pub riders: Vec<u32>,
    pub missions: Vec<u32>,
    pub favour: Vec<(u32, u32, u32, u32)>,
}

const VERSION: u8 = 1;

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: usize) {
        self.0.extend_from_slice(&(v.min(u16::MAX as usize) as u16).to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn vec3(&mut self, v: [f64; 3]) {
        v.iter().for_each(|&c| self.f64(c));
    }
    fn str(&mut self, s: &str) {
        let b = &s.as_bytes()[..s.len().min(1024)];
        self.u16(b.len());
        self.0.extend_from_slice(b);
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_at_checked(N)?;
        self.0 = rest;
        head.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u16(&mut self) -> Option<usize> {
        self.take::<2>().map(|b| usize::from(u16::from_le_bytes(b)))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take::<4>().map(u32::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take::<4>().map(f32::from_le_bytes).filter(|v| v.is_finite())
    }
    fn f64(&mut self) -> Option<f64> {
        self.take::<8>().map(f64::from_le_bytes).filter(|v| v.is_finite())
    }
    fn vec3(&mut self) -> Option<[f64; 3]> {
        Some([self.f64()?, self.f64()?, self.f64()?])
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u16()?;
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        String::from_utf8(head.to_vec()).ok()
    }
    /// A count, no more than `max` (a message from a client is checked by nothing).
    fn count(&mut self, max: usize) -> Option<usize> {
        self.u16().filter(|&n| n <= max)
    }
}

pub fn encode_fx(fx: &Fx) -> Vec<u8> {
    let mut w = Writer(Vec::with_capacity(64));
    w.u8(VERSION);
    let shots = &fx.shots[..fx.shots.len().min(MAX_SHOTS)];
    w.u16(shots.len());
    for &(kind, at, was) in shots {
        w.u8(kind);
        w.vec3(at);
        w.vec3(was);
    }
    let beams = &fx.beams[..fx.beams.len().min(MAX_BEAMS)];
    w.u16(beams.len());
    for &(from, to) in beams {
        w.vec3(from);
        w.vec3(to);
    }
    let swings = &fx.swings[..fx.swings.len().min(MAX_IDS)];
    w.u16(swings.len());
    for &(id, t) in swings {
        w.u32(id);
        w.f32(t);
    }
    for ids in [&fx.riders, &fx.missions] {
        let ids = &ids[..ids.len().min(MAX_IDS)];
        w.u16(ids.len());
        ids.iter().for_each(|&id| w.u32(id));
    }
    let favour = &fx.favour[..fx.favour.len().min(MAX_IDS)];
    w.u16(favour.len());
    for &(id, level, into, need) in favour {
        w.u32(id);
        w.u32(level);
        w.u32(into);
        w.u32(need);
    }
    w.0
}

pub fn decode_fx(data: &[u8]) -> Option<Fx> {
    let mut r = Reader(data);
    if r.u8()? != VERSION {
        return None;
    }
    let mut fx = Fx::default();
    for _ in 0..r.count(MAX_SHOTS)? {
        fx.shots.push((r.u8()?, r.vec3()?, r.vec3()?));
    }
    for _ in 0..r.count(MAX_BEAMS)? {
        fx.beams.push((r.vec3()?, r.vec3()?));
    }
    for _ in 0..r.count(MAX_IDS)? {
        fx.swings.push((r.u32()?, r.f32()?));
    }
    for _ in 0..r.count(MAX_IDS)? {
        fx.riders.push(r.u32()?);
    }
    for _ in 0..r.count(MAX_IDS)? {
        fx.missions.push(r.u32()?);
    }
    for _ in 0..r.count(MAX_IDS)? {
        fx.favour.push((r.u32()?, r.u32()?, r.u32()?, r.u32()?));
    }
    r.0.is_empty().then_some(fx)
}

/// A doll inventory screen as its viewer's client needs it: its title, the doll's slot count, which
/// of them the screen shows from where, and which of those are empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub title: String,
    pub total: usize,
    pub offset: usize,
    pub view: usize,
    pub empty: Vec<bool>,
    pub switch_window: Option<i32>,
}

/// None: the screen closed.
pub fn encode_menu(menu: Option<&Menu>) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    let Some(m) = menu else { return w.0 };
    w.str(&m.title);
    w.u16(m.total);
    w.u16(m.offset);
    w.u16(m.view);
    let empty = &m.empty[..m.empty.len().min(256)];
    w.u16(empty.len());
    empty.iter().for_each(|&e| w.u8(u8::from(e)));
    if let Some(window) = m.switch_window {
        w.u32(window as u32);
    }
    w.0
}

/// The screen, or None for a closed one; Err for a message that doesn't read.
pub fn decode_menu(data: &[u8]) -> Result<Option<Menu>, ()> {
    if data.is_empty() {
        return Ok(None);
    }
    let mut r = Reader(data);
    let read = |r: &mut Reader<'_>| -> Option<Menu> {
        let title = r.str()?;
        let (total, offset, view) = (r.u16()?, r.u16()?, r.u16()?);
        let n = r.count(256)?;
        let empty = (0..n).map(|_| r.u8().map(|b| b != 0)).collect::<Option<Vec<bool>>>()?;
        let switch_window = if r.0.is_empty() { None } else { Some(r.u32()? as i32) };
        Some(Menu { title, total, offset, view, empty, switch_window })
    };
    let menu = read(&mut r).ok_or(())?;
    if r.0.is_empty() { Ok(Some(menu)) } else { Err(()) }
}

/// Aya's photo: from her eyes, turned as she looks.
pub fn encode_photo(eye: [f64; 3], yaw: f32, pitch: f32) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.vec3(eye);
    w.f32(yaw);
    w.f32(pitch);
    w.0
}

pub fn decode_photo(data: &[u8]) -> Option<([f64; 3], f32, f32)> {
    let mut r = Reader(data);
    let photo = (r.vec3()?, r.f32()?, r.f32()?);
    r.0.is_empty().then_some(photo)
}

/// Rows a wheel turn scrolls: a client says, and no more than three at a time are believed.
pub fn decode_scroll(data: &[u8]) -> Option<i32> {
    match data {
        [b] => Some(i32::from(*b as i8).clamp(-3, 3)).filter(|&r| r != 0),
        _ => None,
    }
}

pub fn encode_scroll(rows: i32) -> Vec<u8> {
    vec![rows.clamp(-3, 3) as i8 as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_dolls_show_reaches_a_client_as_it_was() {
        let fx = Fx {
            shots: vec![(0, [1.5, 64.0, -3.25], [1.0, 64.0, -3.0]), (1, [-1e6, 5.0, 2e6], [0.0; 3])],
            beams: vec![([0.0, 70.0, 0.0], [4.0, 71.0, -2.0])],
            swings: vec![(7, 0.5)],
            riders: vec![3, 9],
            missions: vec![11],
            favour: vec![(7, 2, 125, 306), (3, 0, 0, 100)],
        };
        assert_eq!(decode_fx(&encode_fx(&fx)), Some(fx));
        assert_eq!(decode_fx(&encode_fx(&Fx::default())), Some(Fx::default()));
    }

    #[test]
    fn a_full_message_stays_within_what_one_may_carry() {
        let fx = Fx {
            shots: vec![(1, [0.0; 3], [0.0; 3]); 1000],
            beams: vec![([0.0; 3], [0.0; 3]); 100],
            swings: vec![(1, 0.1); 200],
            riders: vec![1; 50],
            missions: vec![1; 10],
            favour: vec![(1, 1, 1, 1); 200],
        };
        let bytes = encode_fx(&fx);
        assert!(bytes.len() <= 30_000, "{}", bytes.len());
        let back = decode_fx(&bytes).unwrap();
        assert_eq!((back.shots.len(), back.beams.len()), (MAX_SHOTS, MAX_BEAMS));
    }

    #[test]
    fn a_broken_message_is_dropped() {
        let bytes = encode_fx(&Fx { riders: vec![1, 2], ..Fx::default() });
        assert_eq!(decode_fx(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode_fx(&[]), None);
        assert_eq!(decode_fx(&[9]), None);
        let mut nan = encode_fx(&Fx { swings: vec![(1, 0.5)], ..Fx::default() });
        // The swing's progress: just before the three empty lists' counts.
        let at = nan.len() - 2 * 3;
        nan[at - 4..at].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(decode_fx(&nan), None);
    }

    #[test]
    fn a_doll_screen_reaches_its_viewer() {
        let m = Menu { title: "Horai Doll".into(), total: 18, offset: 0, view: 18, empty: vec![true, false, true], switch_window: Some(9) };
        assert_eq!(decode_menu(&encode_menu(Some(&m))), Ok(Some(m)));
        assert_eq!(decode_menu(&encode_menu(None)), Ok(None));
        assert_eq!(decode_menu(&[1, 0]), Err(()));
    }

    #[test]
    fn a_photo_and_a_scroll_read_back() {
        assert_eq!(decode_photo(&encode_photo([1.0, 2.0, 3.0], 90.0, -10.0)), Some(([1.0, 2.0, 3.0], 90.0, -10.0)));
        assert_eq!(decode_scroll(&encode_scroll(-1)), Some(-1));
        assert_eq!(decode_scroll(&encode_scroll(40)), Some(3));
        assert_eq!(decode_scroll(&[0]), None);
        assert_eq!(decode_scroll(&[1, 2]), None);
    }
}

use crate::Contact;
use crate::Engine;

pub trait Game: Sized + 'static {
    const DT: f32;
    fn new(engine: &mut Engine<Self>) -> Self;
    fn update(&mut self, engine: &mut Engine<Self>);
    fn handle_collisions(
        &mut self,
        engine: &mut Engine<Self>,
        displacements: impl Iterator<Item = Contact>,
        triggers: impl Iterator<Item = Contact>,
    );
    fn render(&mut self, engine: &mut Engine<Self>);
}

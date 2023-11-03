use crate::Contact;
use crate::Engine;

pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine<Self>) -> Self;
    fn update(&mut self, engine: &mut Engine<Self>);
    fn handle_collisions(
        &mut self,
        engine: &mut Engine<Self>,
        contacts: impl Iterator<Item = Contact>,
    );
    fn handle_triggers(
        &mut self,
        engine: &mut Engine<Self>,
        contacts: impl Iterator<Item = Contact>,
    );
    fn render(&mut self, engine: &mut Engine<Self>);
}

use crate::Contact;
use crate::Engine;

pub trait Game: Sized + 'static {
    type Tag: TagType;
    fn new(engine: &mut Engine<Self>) -> Self;
    fn update(&mut self, engine: &mut Engine<Self>);
    fn handle_collisions(
        &mut self,
        engine: &mut Engine<Self>,
        contacts: impl Iterator<Item = Contact<Self::Tag>>,
    );
    fn handle_triggers(
        &mut self,
        engine: &mut Engine<Self>,
        contacts: impl Iterator<Item = Contact<Self::Tag>>,
    );
    fn render(&mut self, engine: &mut Engine<Self>);
}

pub trait TagType: Copy + Eq + Ord {}

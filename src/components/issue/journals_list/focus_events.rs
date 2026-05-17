use ratatui::layout::Position;

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

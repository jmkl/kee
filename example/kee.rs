use kee::{Event, Kee, TKeePair, kpairs};

fn main() -> anyhow::Result<()> {
    let keypairs = kpairs! {
        (M-1 			=> app::Move),
        (M-2 			=> app::Minimize),
        (C-S-b 		=> app::ControlShiftB),
    };

    Kee::new()
        .register_hotkeys(keypairs)?
        .on_message(|event| match event {
            Event::Keys(k, f) => {
                println!("{} {}", k, f);
            }
            _ => {}
        })
        .run();

    Ok(())
}

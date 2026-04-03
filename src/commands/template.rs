use anyhow::Result;

pub fn run() -> Result<()> {
    println!(
        "name: Example EQ\npreamp: -2.5\nfilter: lowshelf, 28, 2.2, 0.917\nfilter: peak, 223, -6.6, 0.412\nfilter: peak, 791, 2.4, 1.277\nfilter: peak, 2335, -0.9, 1.414\nfilter: peak, 2451, 0.5, 2.998\nfilter: peak, 3596, -3.0, 2.133\nfilter: peak, 4868, 1.6, 1.826"
    );
    Ok(())
}

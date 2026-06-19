use messages::Value;

use crate::{bang_bang, display};

#[embassy_executor::task]
pub async fn task() {
    display::add_text("Water tank", Value::None).await;
    display::add_text("Pump", Value::Analog(26)).await;
    display::add_text("Fill level", Value::Analog(27)).await;
    display::add_text("Source (NC)", Value::OffOn(20)).await;
    display::add_text("Drain  (NO)", Value::OnOff(19)).await;

    bang_bang::set_input(27).await;
    bang_bang::set_output(6).await;
    bang_bang::set_lower_limit(45).await;
    bang_bang::set_upper_limit(50).await;
}

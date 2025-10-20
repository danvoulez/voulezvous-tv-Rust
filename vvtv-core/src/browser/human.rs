use std::ops::RangeInclusive;
use std::time::Duration;

use rand::distributions::Uniform;
use rand::rngs::ThreadRng;
use rand::{thread_rng, Rng};
use tokio::time::sleep;

use chromiumoxide::element::Element;
use chromiumoxide::layout::Point;
use chromiumoxide::page::Page;

use crate::config::HumanSimulationSection;

use super::error::{BrowserError, BrowserResult};

#[derive(Debug, Clone)]
pub struct HumanMotionPlan {
    pub events: Vec<MotionEvent>,
}

#[derive(Debug, Clone)]
pub enum MotionEvent {
    Move { point: Point, delay: Duration },
    Pause(Duration),
    Scroll { delta_y: f64 },
    Typing { ch: char, delay: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPhase {
    Approach,
    Targeting,
    Activation,
    Idle,
}

#[derive(Debug)]
pub struct HumanMotionController {
    config: HumanSimulationSection,
    last_point: Option<Point>,
    rng: ThreadRng,
}

impl HumanMotionController {
    pub fn new(config: HumanSimulationSection) -> Self {
        Self {
            config,
            last_point: None,
            rng: thread_rng(),
        }
    }

    pub async fn idle(&mut self) -> BrowserResult<()> {
        let delay = self.random_duration(self.config.idle_duration_ms);
        sleep(delay).await;
        Ok(())
    }

    pub async fn move_to_element(
        &mut self,
        page: &Page,
        element: &Element,
    ) -> BrowserResult<Point> {
        let bbox = element.bounding_box().await.map_err(|err| {
            BrowserError::Unexpected(format!("failed to get element bounding box: {err}"))
        })?;
        let jitter = self.config.mouse_jitter_px as f64;
        let target_x = bbox.x + self.rng.gen_range(0.3..0.7) * bbox.width;
        let target_y = bbox.y + self.rng.gen_range(0.2..0.6) * bbox.height;
        let target = Point::new(
            target_x + self.random_offset(jitter),
            target_y + self.random_offset(jitter),
        );
        let plan = self.plan_motion(target);
        self.execute_motion(page, &plan).await?;
        self.last_point = Some(target);
        Ok(target)
    }

    pub async fn click_element(&mut self, page: &Page, element: &Element) -> BrowserResult<()> {
        self.move_to_element(page, element).await?;
        let hesitation = self.random_duration(self.config.click_hesitation_ms);
        sleep(hesitation).await;
        element
            .click()
            .await
            .map_err(|err| BrowserError::Unexpected(format!("failed to click element: {err}")))?;
        let dwell = self.random_duration(self.config.click_duration_ms);
        sleep(dwell).await;
        Ok(())
    }

    pub async fn type_text(&mut self, element: &Element, text: &str) -> BrowserResult<()> {
        element.click().await.map_err(|err| {
            BrowserError::Unexpected(format!("failed to focus element before typing: {err}"))
        })?;
        for ch in text.chars() {
            element.type_str(ch.to_string()).await.map_err(|err| {
                BrowserError::Unexpected(format!("failed to type character: {err}"))
            })?;
            let delay = self.typing_delay();
            sleep(delay).await;
        }
        Ok(())
    }

    pub async fn random_idle(&mut self) -> BrowserResult<()> {
        if self.config.ociosidade_frequency[1] == 0 {
            return Ok(());
        }
        let freq_range = RangeInclusive::new(
            self.config.ociosidade_frequency[0],
            self.config.ociosidade_frequency[1],
        );
        if self.rng.gen_range(freq_range) % 2 == 0 {
            self.idle().await?
        }
        Ok(())
    }

    pub async fn scroll_by(&mut self, page: &Page, delta: f64) -> BrowserResult<()> {
        let duration = self.random_duration(self.config.scroll_pause_ms);
        let js = format!("window.scrollBy({{ top: {delta}, behavior: 'smooth' }});");
        page.evaluate(js.as_str()).await.map_err(|err| {
            BrowserError::Unexpected(format!("failed to execute scroll script: {err}"))
        })?;
        sleep(duration).await;
        Ok(())
    }

    fn plan_motion(&mut self, target: Point) -> HumanMotionPlan {
        let start = self.last_point.unwrap_or_else(|| Point::new(0.0, 0.0));
        let distance = ((target.x - start.x).powi(2) + (target.y - start.y).powi(2)).sqrt();
        let speed = self
            .rng
            .gen_range(self.config.mouse_speed_min_px_s..=self.config.mouse_speed_max_px_s)
            as f64;
        let duration_secs = (distance / speed).max(0.08);
        let steps = (duration_secs * 60.0).clamp(12.0, 48.0) as usize;
        let mut events = Vec::with_capacity(steps + 1);
        for idx in 0..steps {
            let t = idx as f64 / steps as f64;
            let eased = ease_in_out_cubic(t);
            let intermediate = Point::new(
                start.x + (target.x - start.x) * eased + self.random_offset(1.2),
                start.y + (target.y - start.y) * eased + self.random_offset(1.2),
            );
            let delay = Duration::from_secs_f64(duration_secs / steps as f64);
            events.push(MotionEvent::Move {
                point: intermediate,
                delay,
            });
        }
        HumanMotionPlan { events }
    }

    async fn execute_motion(&mut self, page: &Page, plan: &HumanMotionPlan) -> BrowserResult<()> {
        for event in &plan.events {
            match event {
                MotionEvent::Move { point, delay } => {
                    page.move_mouse(*point).await.map_err(|err| {
                        BrowserError::Unexpected(format!("failed to move mouse: {err}"))
                    })?;
                    sleep(*delay).await;
                }
                MotionEvent::Pause(duration) => sleep(*duration).await,
                MotionEvent::Scroll { delta_y } => {
                    let js = format!("window.scrollBy({{ top: {delta_y}, behavior: 'smooth' }});");
                    page.evaluate(js.as_str()).await.map_err(|err| {
                        BrowserError::Unexpected(format!(
                            "failed to execute scroll during motion: {err}"
                        ))
                    })?;
                }
                MotionEvent::Typing { .. } => {}
            }
        }
        Ok(())
    }

    fn typing_delay(&mut self) -> Duration {
        let cadence_range = RangeInclusive::new(
            self.config.typing_cadence_cpm[0],
            self.config.typing_cadence_cpm[1],
        );
        let cadence = self.rng.gen_range(cadence_range).max(60) as f64;
        let chars_per_second = cadence / 60.0;
        let base_delay = 1.0 / chars_per_second;
        let jitter_range = RangeInclusive::new(
            self.config.typing_jitter_ms[0],
            self.config.typing_jitter_ms[1],
        );
        let jitter_ms = self.rng.gen_range(jitter_range);
        Duration::from_secs_f64(base_delay + jitter_ms as f64 / 1000.0)
    }

    fn random_duration(&mut self, bounds: [u32; 2]) -> Duration {
        let ms = self.rng.gen_range(bounds[0]..=bounds[1]) as u64;
        Duration::from_millis(ms)
    }

    fn random_offset(&mut self, max: f64) -> f64 {
        if max <= 0.0 {
            return 0.0;
        }
        let distribution = Uniform::new_inclusive(-max, max);
        self.rng.sample(distribution)
    }
}

fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

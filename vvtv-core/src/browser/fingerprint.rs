use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use chromiumoxide::page::Page;

use crate::config::FingerprintSection;

use super::error::{BrowserError, BrowserResult};

#[derive(Debug, Clone)]
pub struct FingerprintMasker {
    config: FingerprintSection,
}

impl FingerprintMasker {
    pub fn new(config: FingerprintSection) -> Self {
        Self { config }
    }

    pub async fn apply(&self, page: &Page) -> BrowserResult<()> {
        if self.config.enable_canvas_noise {
            self.inject_canvas_noise(page).await?;
        }
        if self.config.enable_webgl_mask {
            self.mask_webgl(page).await?;
        }
        if self.config.enable_audio_mask {
            self.mask_audio_context(page).await?;
        }
        Ok(())
    }

    async fn inject_canvas_noise(&self, page: &Page) -> BrowserResult<()> {
        let min = self.config.canvas_noise_range[0];
        let max = self.config.canvas_noise_range[1];
        let script = format!(
            r#"
            (() => {{
                const randomInt = (min, max) => {{
                    return Math.floor(Math.random() * (max - min + 1)) + min;
                }};
                const originalToDataURL = HTMLCanvasElement.prototype.toDataURL;
                HTMLCanvasElement.prototype.toDataURL = function() {{
                    try {{
                        const ctx = this.getContext('2d');
                        if (ctx) {{
                            const imageData = ctx.getImageData(0, 0, this.width, this.height);
                            for (let i = 0; i < imageData.data.length; i += 4) {{
                                const delta = randomInt({min}, {max});
                                imageData.data[i] = Math.min(255, Math.max(0, imageData.data[i] + delta));
                            }}
                            ctx.putImageData(imageData, 0, 0);
                        }}
                    }} catch (_) {{}}
                    return originalToDataURL.apply(this, arguments);
                }};
            }})();
            "#
        );
        page.evaluate_on_new_document(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(script)
                .build()
                .map_err(|err| BrowserError::Configuration(err.to_string()))?,
        )
        .await?;
        Ok(())
    }

    async fn mask_webgl(&self, page: &Page) -> BrowserResult<()> {
        let vendor = self
            .config
            .webgl_vendor
            .clone()
            .unwrap_or_else(|| "Intel Inc.".to_string());
        let renderer = self
            .config
            .webgl_renderer
            .clone()
            .unwrap_or_else(|| "Intel Iris OpenGL Engine".to_string());
        let script = format!(
            r#"
            (() => {{
                const spoofParam = (proto) => {{
                    if (!proto || !proto.getParameter) {{
                        return;
                    }}
                    const original = proto.getParameter;
                    proto.getParameter = function(param) {{
                        if (param === 37445) {{
                            return '{vendor}';
                        }}
                        if (param === 37446) {{
                            return '{renderer}';
                        }}
                        return original.apply(this, arguments);
                    }};
                }};
                spoofParam(WebGLRenderingContext?.prototype);
                spoofParam(WebGL2RenderingContext?.prototype);
            }})();
            "#
        );
        page.evaluate_on_new_document(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(script)
                .build()
                .map_err(|err| BrowserError::Configuration(err.to_string()))?,
        )
        .await?;
        Ok(())
    }

    async fn mask_audio_context(&self, page: &Page) -> BrowserResult<()> {
        let noise = self.config.audio_noise;
        let script = format!(
            r#"
            (() => {{
                const noiseLevel = {noise};
                const origGetChannelData = AudioBuffer?.prototype?.getChannelData;
                if (!origGetChannelData) {{
                    return;
                }}
                AudioBuffer.prototype.getChannelData = function(channel) {{
                    const data = origGetChannelData.call(this, channel);
                    if (data) {{
                        for (let i = 0; i < data.length; i++) {{
                            data[i] = data[i] + (Math.random() * noiseLevel - noiseLevel / 2);
                        }}
                    }}
                    return data;
                }};
            }})();
            "#
        );
        page.evaluate_on_new_document(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(script)
                .build()
                .map_err(|err| BrowserError::Configuration(err.to_string()))?,
        )
        .await?;
        Ok(())
    }
}

# VVTV Streaming Architecture Explained
## How the Autonomous 24/7 Streaming System Works

### 🎯 **Overview: What VVTV Actually Does**

VVTV is like having a **fully automated TV station** that:
1. **Finds content** on the internet (like a human browsing)
2. **Downloads and processes** videos automatically
3. **Creates a continuous stream** 24/7 like a TV channel
4. **Broadcasts live** to viewers around the world
5. **Adapts programming** based on AI and viewer behavior

Think of it as **Netflix + Twitch + AI** all combined into one autonomous system.

---

## 🔄 **The Complete Flow: From Discovery to Viewer**

```
┌─────────────────────────────────────────────────────────────────┐
│                    VVTV STREAMING PIPELINE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. DISCOVERY          2. PROCESSING         3. STREAMING       │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐     │
│  │   Browser   │ ──── │   FFmpeg    │ ──── │   NGINX     │     │
│  │ Automation  │      │ Processing  │      │   RTMP      │     │
│  │             │      │             │      │             │     │
│  │ • Searches  │      │ • Downloads │      │ • Encodes   │     │
│  │ • Finds     │      │ • Converts  │      │ • Streams   │     │
│  │ • Evaluates │      │ • Normalizes│      │ • Broadcasts│     │
│  └─────────────┘      └─────────────┘      └─────────────┘     │
│         │                      │                      │         │
│         ▼                      ▼                      ▼         │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐     │
│  │    Plans    │      │   Queue     │      │     CDN     │     │
│  │  Database   │      │  Database   │      │ Distribution│     │
│  └─────────────┘      └─────────────┘      └─────────────┘     │
│                                                     │           │
│                                                     ▼           │
│                                            ┌─────────────┐     │
│                                            │   VIEWERS   │     │
│                                            │  Worldwide  │     │
│                                            └─────────────┘     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🕵️ **Phase 1: Content Discovery (The "Curator")**

### **How It Finds Content**
```rust
// Simplified version of what happens
async fn discover_content() {
    // 1. Open a real browser (Chromium)
    let browser = launch_browser().await;
    
    // 2. Search for content (like a human would)
    browser.navigate("https://example-video-site.com").await;
    browser.search("creative commons music").await;
    
    // 3. Evaluate each video found
    for video in search_results {
        // 4. Play the video BEFORE downloading (Play-Before-Download)
        browser.click_play(video).await;
        
        // 5. Check quality in real-time
        let quality = analyze_stream_quality().await;
        
        // 6. Ask local LLM: "Is this good content?"
        let llm_assessment = llm.analyze_content(video).await;
        
        // 7. If good, save as a "plan" (not download yet!)
        if quality.is_good() && llm_assessment.is_positive() {
            save_plan(video).await;
        }
    }
}
```

### **What Makes It Smart**
- **Human Simulation**: Moves mouse naturally, types like a human, scrolls organically
- **Play-Before-Download**: Actually plays the video to verify it's HD quality
- **LLM Analysis**: Local AI evaluates if content fits the channel's style
- **No APIs**: Works with any website, no special permissions needed

---

## 🎬 **Phase 2: Content Processing (The "Factory")**

### **From Raw Video to Broadcast-Ready**
```rust
async fn process_content(plan: Plan) {
    // 1. Download the actual video file
    let raw_video = download_video(plan.url).await;
    
    // 2. Analyze what we got
    let analysis = ffprobe_analysis(raw_video).await;
    
    // 3. Convert to broadcast standard
    let processed_video = ffmpeg_process(raw_video, ProcessingConfig {
        resolution: "1920x1080",
        bitrate: "4500k",
        audio_normalization: "EBU R128 -14 LUFS", // Broadcast standard
        format: "mp4",
    }).await;
    
    // 4. Quality check
    let quality_score = vmaf_analysis(processed_video, raw_video).await;
    
    // 5. If quality is good, add to queue
    if quality_score > 85.0 {
        add_to_broadcast_queue(processed_video).await;
    }
}
```

### **What Happens Here**
- **Download**: Gets the actual video file from the internet
- **Transcode**: Converts to consistent format (1080p, proper bitrate)
- **Audio Normalize**: Makes all audio the same volume (broadcast standard)
- **Quality Check**: Uses VMAF (Netflix's quality metric) to verify it's good
- **Queue**: Adds to the playlist for broadcasting

---

## 📺 **Phase 3: Live Streaming (The "Broadcaster")**

### **Creating the Continuous Stream**
```rust
async fn broadcast_loop() {
    loop {
        // 1. Get next video from queue
        let next_video = queue.get_next_video().await;
        
        // 2. Start FFmpeg encoder
        let encoder = FFmpegEncoder::new()
            .input(next_video.path)
            .output("rtmp://localhost/live/main")
            .video_codec("libx264")
            .audio_codec("aac")
            .bitrate("4500k")
            .start().await;
        
        // 3. Monitor while playing
        while encoder.is_running() {
            let health = encoder.get_health().await;
            
            if health.has_issues() {
                // Handle problems automatically
                restart_encoder().await;
            }
            
            sleep(Duration::from_secs(1)).await;
        }
        
        // 4. Video finished, loop to next one
    }
}
```

### **The Streaming Stack**
```
┌─────────────────────────────────────────────────────────────┐
│                    STREAMING STACK                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│  │   Video     │    │   FFmpeg    │    │   NGINX     │    │
│  │   Files     │───▶│   Encoder   │───▶│   RTMP      │    │
│  │             │    │             │    │             │    │
│  │ • MP4 Files │    │ • Encodes   │    │ • Receives  │    │
│  │ • Ready to  │    │ • Live      │    │ • RTMP      │    │
│  │   Play      │    │ • Stream    │    │ • Creates   │    │
│  └─────────────┘    └─────────────┘    │ • HLS       │    │
│                                        └─────────────┘    │
│                                               │            │
│                                               ▼            │
│                                    ┌─────────────┐        │
│                                    │     CDN     │        │
│                                    │ Distribution│        │
│                                    │             │        │
│                                    │ • Global    │        │
│                                    │ • Cached    │        │
│                                    │ • Fast      │        │
│                                    └─────────────┘        │
│                                           │                │
│                                           ▼                │
│                                  ┌─────────────┐          │
│                                  │   VIEWERS   │          │
│                                  │   Browser   │          │
│                                  │   Mobile    │          │
│                                  │     TV      │          │
│                                  └─────────────┘          │
│                                                           │
└─────────────────────────────────────────────────────────────┘
```

---

## 🌐 **Phase 4: Global Distribution (The "CDN")**

### **How Viewers Actually Watch**

1. **RTMP to HLS Conversion**
   ```bash
   # NGINX-RTMP converts the stream to HLS segments
   # Input: rtmp://localhost/live/main
   # Output: /vvtv/broadcast/hls/main.m3u8 + segment files
   ```

2. **HLS Segments**
   ```
   # What gets created:
   /vvtv/broadcast/hls/
   ├── main.m3u8          # Playlist file
   ├── segment_001.ts     # 4-second video chunk
   ├── segment_002.ts     # 4-second video chunk
   ├── segment_003.ts     # 4-second video chunk
   └── ...
   ```

3. **CDN Distribution**
   ```
   Origin Server (AWS) ──▶ BunnyCDN ──▶ Edge Servers ──▶ Viewers
   ```

### **What Viewers See**
- **URL**: `https://stream.voulezvous.tv/live.m3u8`
- **Player**: Any HLS-compatible player (VLC, web browsers, mobile apps)
- **Experience**: Continuous stream like watching TV

---

## 🧠 **The Intelligence Layer (LLM + Business Logic)**

### **How AI Makes It Smart**
```rust
// Business Logic decides what to play next
async fn select_next_content() {
    // 1. Get available content
    let candidates = get_available_content().await;
    
    // 2. Apply business rules
    let filtered = business_logic.apply_rules(candidates).await;
    
    // 3. Ask LLM for refinement (optional)
    let llm_suggestions = llm.rerank_content(filtered).await;
    
    // 4. Use Gumbel-Top-k algorithm for final selection
    let selected = gumbel_top_k_selection(llm_suggestions).await;
    
    return selected;
}
```

### **What Makes Decisions**
- **Time of Day**: Different content for morning vs. night
- **Viewer Behavior**: Adapts based on what people watch
- **Content Diversity**: Ensures variety, not repetitive
- **Quality Scores**: Prioritizes higher quality content
- **LLM Insights**: AI suggests what flows well together

---

## 🔧 **Technical Deep Dive: The Protocols**

### **RTMP (Real-Time Messaging Protocol)**
```
What: Protocol for live streaming
Used: FFmpeg → NGINX-RTMP
Purpose: Reliable live video transmission
```

### **HLS (HTTP Live Streaming)**
```
What: Apple's streaming protocol
Used: NGINX-RTMP → CDN → Viewers
Purpose: Adaptive streaming over HTTP
Benefits: Works everywhere, adaptive quality
```

### **The Conversion Process**
```
MP4 File ──FFmpeg──▶ RTMP Stream ──NGINX──▶ HLS Segments ──CDN──▶ Viewers
```

---

## 🏗️ **Infrastructure Components**

### **On AWS (Your Setup)**
```yaml
Streaming Node (c6i.2xlarge):
  - Runs VVTV application
  - FFmpeg encoding
  - NGINX-RTMP server
  - Content processing

LLM Node (g5.xlarge):
  - Local Ollama server
  - Llama 3.1 model
  - Content analysis
  - Privacy-first AI

Storage:
  - EFS: Active content
  - S3: Archive storage
  - RDS: Databases

CDN:
  - BunnyCDN: Global distribution
  - Adult-content friendly
  - Low latency worldwide
```

### **Data Flow**
```
Internet ──▶ Browser ──▶ Plans DB ──▶ Processing ──▶ Queue DB ──▶ Streaming ──▶ CDN ──▶ Viewers
    ▲                                                                                      │
    └─────────────────── LLM Analysis ◀─────────────────────────────────────────────────┘
```

---

## 🎮 **How You Control It**

### **Command Line Interface**
```bash
# Check system status
vvtvctl status

# See what's in the queue
vvtvctl queue show

# Check streaming health
vvtvctl streaming status

# Force content discovery
vvtvctl discover --query "creative commons music" --max-plans 10

# Adjust business logic
vvtvctl business-logic set exploration.epsilon=0.15

# Emergency controls
vvtvctl streaming emergency --inject 5 --reason "low_buffer"
```

### **What You Can Monitor**
- **Stream Health**: Is it broadcasting?
- **Queue Status**: How much content is ready?
- **Discovery**: Is it finding new content?
- **Quality**: Are viewers getting good quality?
- **AI Status**: Is the LLM working?

---

## 🚨 **What Can Go Wrong (And How It Handles It)**

### **Common Issues & Auto-Recovery**

1. **Stream Goes Down**
   ```rust
   // Auto-restart encoder
   if stream_health.is_down() {
       restart_encoder().await;
       inject_emergency_content().await;
   }
   ```

2. **Queue Runs Empty**
   ```rust
   // Emergency loop content
   if queue.buffer_minutes() < 30 {
       activate_emergency_loop().await;
       trigger_urgent_discovery().await;
   }
   ```

3. **LLM Fails**
   ```rust
   // Fallback to deterministic mode
   if llm.circuit_breaker.is_open() {
       use_deterministic_selection().await;
   }
   ```

4. **Quality Issues**
   ```rust
   // Auto-adjust encoding
   if quality_score < threshold {
       adjust_encoding_preset().await;
       restart_with_lower_settings().await;
   }
   ```

---

## 🎯 **Why This Architecture Works**

### **For Adult Content**
- **Privacy**: Local LLM, no external AI calls
- **Compliance**: Age verification, geo-blocking
- **Quality**: Ensures HD content only
- **Reliability**: 24/7 autonomous operation

### **For Business**
- **Scalable**: Handles growing audience
- **Cost-Effective**: Optimized for AWS costs
- **Monetizable**: Built-in revenue tracking
- **Maintainable**: Comprehensive monitoring

### **For Users**
- **Always On**: Never goes offline
- **High Quality**: Consistent HD streaming
- **Fast**: Global CDN distribution
- **Adaptive**: Gets better over time

---

## 🔮 **The Magic: How It All Comes Together**

Imagine you're watching the stream:

1. **Right Now**: You're watching a video that was discovered by AI browsing the internet yesterday
2. **Behind the Scenes**: The system is already downloading tomorrow's content
3. **In Real-Time**: AI is analyzing what you and others are watching to improve future selections
4. **Continuously**: New content is being discovered, processed, and queued automatically

**It's like having a TV station that never sleeps, never repeats content, and gets smarter every day.**

The beauty is that once deployed, it runs completely autonomously. You just monitor it and occasionally adjust the business logic parameters to fine-tune the programming.

---

## 🚀 **Your Next Steps**

1. **Deploy on AWS** using the deployment script
2. **Configure the CDN** (BunnyCDN for adult content)
3. **Set up monitoring** (Grafana dashboards)
4. **Tune the business logic** (content preferences)
5. **Watch it run** and make adjustments as needed

The system is designed to be **"set it and forget it"** - once running, it should operate autonomously 24/7 with minimal intervention.

**That's the magic of VVTV: Autonomous, intelligent, always-on streaming television powered by AI.** 🎬✨
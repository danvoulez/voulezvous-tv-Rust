# VVTV Budget Mac Mini Deployment
## Professional Streaming for ~$30/month with 3 Mac Minis

### 🎯 **Perfect Setup for Your Situation**

**3 Mac Minis + 10 Viewers = Ideal Budget Deployment**

You have the perfect hardware setup! Mac Minis are actually excellent for streaming - Apple Silicon is incredibly efficient for video encoding.

---

## 🏗️ **Optimal 3-Node Architecture**

```
┌─────────────────────────────────────────────────────────────┐
│                  YOUR OFFICE SETUP                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Mac Mini   │    │   Mac Mini   │    │   Mac Mini   │  │
│  │      #1      │    │      #2      │    │      #3      │  │
│  │  (Primary)   │    │    (LLM)     │    │  (Backup)    │  │
│  │              │    │              │    │              │  │
│  │ • Streaming  │    │ • Local AI   │    │ • Failover   │  │
│  │ • Discovery  │    │ • Ollama     │    │ • Storage    │  │
│  │ • Processing │    │ • Analysis   │    │ • Monitor    │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                     │                     │       │
│         └─────────────────────┼─────────────────────┘       │
│                               │                             │
│                    ┌──────────────────┐                    │
│                    │   Tailscale VPN  │                    │
│                    │   (Free Tier)    │                    │
│                    └──────────────────┘                    │
│                               │                             │
└─────────────────────────────────────────────────────────────┘
                                │
                                ▼
                    ┌──────────────────┐
                    │   Railway CDN    │
                    │   ($20/month)    │
                    └──────────────────┘
                                │
                                ▼
                        ┌──────────────┐
                        │   VIEWERS    │
                        │  (10 people) │
                        └──────────────┘
```

---

## 💰 **Total Monthly Cost: ~$30**

```yaml
Hardware Costs:
  - Mac Minis: $0/month (you own them)
  - Internet: $0/month (your existing connection)
  - Electricity: ~$5/month (3 Mac Minis are very efficient)

Cloud Services:
  - Railway CDN: $20/month (adult-friendly, global)
  - Domain: $10/month (voulezvous.tv)
  - Tailscale: $0/month (free for personal use)

Total: $35/month maximum
```

---

## 🖥️ **Mac Mini Role Assignment**

### **Mac Mini #1: Primary Streaming Node**
```yaml
Specs Needed: M1/M2 with 16GB RAM (ideal)
Role: Main streaming and content processing

Services:
  - VVTV Core Application
  - FFmpeg encoding (hardware accelerated)
  - NGINX-RTMP server
  - Content discovery (browser automation)
  - Queue management

Performance:
  - Can easily handle 1080p encoding
  - Hardware acceleration via VideoToolbox
  - Low power consumption
  - Excellent for 24/7 operation
```

### **Mac Mini #2: Local LLM Node**
```yaml
Specs Needed: M1/M2 with 8GB+ RAM
Role: Private AI processing

Services:
  - Ollama server
  - Llama 3.1 8B model (quantized)
  - Content analysis
  - Curation hints

Benefits:
  - Complete privacy (no external AI calls)
  - Perfect for adult content analysis
  - Apple Silicon is surprisingly good for LLM inference
  - No GPU needed - unified memory architecture
```

### **Mac Mini #3: Backup & Storage**
```yaml
Specs Needed: Any Mac Mini (even older Intel)
Role: Failover and storage

Services:
  - Hot backup of primary node
  - Additional storage (external drives)
  - Monitoring and alerting
  - Development/testing environment

Failover:
  - Automatically takes over if primary fails
  - Resurrection protocol ready
  - Maintains service continuity
```

---

## 🌐 **Network Architecture**

### **Tailscale Mesh Network (Free)**
```yaml
Benefits:
  - Secure VPN between all nodes
  - No port forwarding needed
  - Works from anywhere
  - Free for personal use
  - Perfect for your setup

Configuration:
  - All 3 Mac Minis join Tailscale network
  - Primary node accessible as: primary.tailnet-name.ts.net
  - LLM node: llm.tailnet-name.ts.net
  - Backup node: backup.tailnet-name.ts.net
```

### **Railway for CDN ($20/month)**
```yaml
Why Railway:
  - Adult content friendly
  - Simple deployment
  - Global CDN included
  - Affordable pricing
  - Easy setup

Setup:
  - Railway pulls HLS stream from your primary Mac Mini
  - Distributes globally via their CDN
  - Handles all the complexity
  - Perfect for 10-100 viewers
```

---

## ⚙️ **Installation Guide**

### **Step 1: Prepare Mac Minis**

```bash
# On each Mac Mini, install dependencies
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install required tools
brew install ffmpeg nginx sqlite3 git rust

# Install Ollama (on Mac Mini #2 only)
curl -fsSL https://ollama.ai/install.sh | sh
```

### **Step 2: Setup Tailscale**

```bash
# Install Tailscale on all 3 Mac Minis
brew install --cask tailscale

# Start and authenticate (do this on each Mac Mini)
sudo tailscale up

# Set hostnames
sudo tailscale set --hostname vvtv-primary    # Mac Mini #1
sudo tailscale set --hostname vvtv-llm        # Mac Mini #2  
sudo tailscale set --hostname vvtv-backup     # Mac Mini #3
```

### **Step 3: Deploy VVTV**

```bash
# On Mac Mini #1 (Primary)
git clone https://github.com/voulezvous-tv/voulezvous-tv-Rust.git
cd voulezvous-tv-Rust
cargo build --release

# Create directory structure
sudo mkdir -p /vvtv/{data,cache,storage,broadcast,system,vault}
sudo chown -R $(whoami):staff /vvtv

# Copy budget configuration
cp configs/mac_mini_budget.toml /vvtv/system/vvtv.toml
```

---

## 📝 **Budget Configuration**

<function_calls>
<invoke name="fsWrite">
<parameter name="path">configs/mac_mini_budget.toml VVTV Mac Mini Budget Configuration
# Optimized for 3 Mac Minis + 10 viewers + minimal costs

[system]
node_name = "vvtv-mac-primary"
node_role = "broadcast"
environment = "budget_production"

[paths]
base_dir = "/vvtv"
data_dir = "/vvtv/data"
cache_dir = "/vvtv/cache"
storage_dir = "/vvtv/storage"
broadcast_dir = "/vvtv/broadcast"
business_logic = "/vvtv/system/business_logic.yaml"

[limits]
# Budget-optimized limits
buffer_target_hours = 4                   # Smaller buffer to save storage
buffer_warning_hours = 2
buffer_critical_hours = 1

# Conservative concurrency for Mac Mini
max_concurrent_downloads = 1              # One at a time to save bandwidth
max_concurrent_transcodes = 1             # Mac Mini can handle this easily
max_browser_instances = 1                 # Sufficient for discovery

# Resource limits (Mac Mini optimized)
cpu_limit_percent = 70                    # Leave headroom for macOS
memory_limit_gb = 6                       # Conservative for 8GB Mac Mini
disk_warning_percent = 80
disk_critical_percent = 90

# Aggressive cleanup for budget storage
plans_retention_days = 7                  # Short retention
played_retention_hours = 24               # Clean up quickly
logs_retention_days = 3                   # Minimal logs
cache_retention_hours = 6                 # Aggressive cache cleanup

[network]
# Tailscale networking
rtmp_port = 1935
hls_port = 8080
control_port = 9000
llm_endpoint = "http://vvtv-llm.tailnet-name.ts.net:11434"

# Railway CDN
cdn_primary = "railway"
cdn_backup = "local"                      # Fallback to direct connection

[budget]
# Budget-specific settings
max_monthly_bandwidth_gb = 100            # Conservative estimate
storage_cleanup_aggressive = true
quality_vs_cost_balance = "balanced"      # Not highest quality, but good

[quality]
# Balanced quality for budget
target_lufs = -16.0                       # Slightly higher for smaller files
audio_codec = "aac"
audio_bitrate = "128k"                    # Good quality, smaller files

# Video optimized for Mac Mini hardware acceleration
vmaf_threshold = 75                       # Good quality, not perfect
target_resolution = "1080p"
fallback_resolution = "720p"
encoder = "videotoolbox"                  # Mac Mini hardware acceleration

# HLS optimized for low bandwidth
hls_segment_duration = 6                  # Longer segments = less overhead
hls_playlist_length_minutes = 30          # Shorter playlist = less storage

[hardware]
# Mac Mini specific optimizations
use_hardware_acceleration = true
videotoolbox_enabled = true               # Apple Silicon acceleration
cpu_encoding_fallback = true             # Fallback if HW fails
power_efficiency_mode = true              # Optimize for 24/7 operation

[llm]
# Local LLM on Mac Mini #2
provider = "local_ollama"
endpoint = "http://vvtv-llm.tailnet-name.ts.net:11434"
timeout_seconds = 15                      # Shorter timeout for budget
max_concurrent_requests = 1               # Conservative for Mac Mini

# Budget LLM settings
models = ["llama3.1:8b-instruct-q4_K_M"] # Single model to save memory
circuit_breaker_enabled = true
external_calls_disabled = true           # Privacy + cost savings

[discovery]
# Budget discovery settings
max_plans_per_session = 5                 # Fewer plans to save processing
discovery_interval_hours = 6              # Less frequent discovery
browser_headless = true                   # Save resources
parallel_sessions = 1                     # Single session only

[storage]
# Aggressive storage management
cleanup_enabled = true
cleanup_interval_hours = 2
max_storage_gb = 50                       # Conservative limit
archive_to_external = true                # Use external drives

[monitoring]
# Lightweight monitoring
health_check_interval_seconds = 120       # Less frequent checks
metrics_collection_interval_seconds = 300
detailed_logging = false                  # Save storage
performance_profiling = false            # Save CPU

[failover]
# Mac Mini #3 backup configuration
backup_node_enabled = true
backup_node_endpoint = "http://vvtv-backup.tailnet-name.ts.net:9000"
sync_interval_minutes = 30
auto_failover_enabled = true

[railway]
# Railway CDN configuration
enabled = true
origin_url = "http://vvtv-primary.tailnet-name.ts.net:8080"
domain = "stream.voulezvous.tv"
adult_content = true
geo_restrictions = []                     # No restrictions for budget

[cost_optimization]
# Aggressive cost optimization
bandwidth_limiting = true
max_bitrate_kbps = 3000                   # Good quality, controlled bandwidth
adaptive_quality = true                   # Lower quality during low viewership
off_peak_processing = true                # Heavy processing during off-peak
storage_tiering = true                    # Move old content to cheaper storage
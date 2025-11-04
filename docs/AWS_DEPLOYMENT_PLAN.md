# VVTV AWS Deployment Plan
## Adult Content Streaming Platform with Local LLM

### 🎯 **Executive Summary**

This deployment plan addresses the unique requirements of VVTV as an adult content streaming platform on AWS, with emphasis on:
- **Content compliance** and adult-friendly infrastructure
- **Local LLM deployment** for privacy and content analysis
- **High-performance streaming** with global CDN
- **Cost optimization** for 24/7 autonomous operation

---

## 🏗️ **Architecture Overview**

### **Core Infrastructure**
```
┌─────────────────────────────────────────────────────────────┐
│                        AWS DEPLOYMENT                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Primary    │    │   LLM Node   │    │   Storage    │  │
│  │   EC2 c6i    │    │   EC2 g5     │    │   EFS/S3     │  │
│  │   (Streaming)│    │   (Local AI) │    │   (Content)  │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                     │                     │       │
│         └─────────────────────┼─────────────────────┘       │
│                               │                             │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              CloudFront CDN (Adult-Friendly)           │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │ │
│  │  │   US-East   │  │   EU-West   │  │   AP-South  │    │ │
│  │  │   (Primary) │  │   (GDPR)    │  │   (Backup)  │    │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘    │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 🖥️ **EC2 Instance Configuration**

### **Primary Streaming Node**
- **Instance Type:** `c6i.2xlarge` (8 vCPU, 16GB RAM)
- **Storage:** 500GB GP3 SSD + 2TB EFS for content
- **Network:** Enhanced networking, 10 Gbps
- **OS:** Amazon Linux 2023 (optimized for streaming)

**Rationale:** 
- High CPU performance for FFmpeg encoding
- Sufficient RAM for concurrent operations
- Cost-effective for 24/7 operation (~$250/month)

### **Local LLM Node** 
- **Instance Type:** `g5.xlarge` (4 vCPU, 16GB RAM, 1x NVIDIA A10G)
- **Storage:** 200GB GP3 SSD
- **Network:** 10 Gbps to primary node
- **OS:** Deep Learning AMI (Ubuntu 22.04)

**Rationale:**
- GPU acceleration for local LLM inference
- Privacy compliance (no external LLM calls)
- Adult content analysis without data leakage
- Cost: ~$400/month (much cheaper than API calls)

### **Backup/Failover Node**
- **Instance Type:** `c6i.large` (2 vCPU, 4GB RAM)
- **Storage:** 100GB GP3 SSD
- **Purpose:** Hot standby for resurrection protocol

---

## 🧠 **Local LLM Setup**

### **Model Selection for Adult Content**
```yaml
# LLM Configuration for Adult Content Analysis
models:
  primary:
    name: "llama-3.1-8b-instruct"
    quantization: "Q4_K_M"
    context_length: 8192
    use_case: "content_analysis"
    
  secondary:
    name: "phi-3.5-mini-instruct"
    quantization: "Q4_K_M" 
    context_length: 4096
    use_case: "query_enhancement"

deployment:
  framework: "ollama"
  gpu_memory: "12GB"
  cpu_threads: 4
  concurrent_requests: 3
  
privacy:
  no_external_calls: true
  local_only: true
  content_filtering: false  # We handle adult content
  logging_minimal: true
```

### **Installation Script**
```bash
#!/bin/bash
# Install Ollama and models on g5.xlarge

# Install NVIDIA drivers and CUDA
sudo yum update -y
sudo yum install -y gcc kernel-devel-$(uname -r)
wget https://developer.download.nvidia.com/compute/cuda/12.2.0/local_installers/cuda_12.2.0_535.54.03_linux.run
sudo sh cuda_12.2.0_535.54.03_linux.run --silent --toolkit

# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh
sudo systemctl enable ollama
sudo systemctl start ollama

# Pull models
ollama pull llama3.1:8b-instruct-q4_K_M
ollama pull phi3.5:mini-instruct-q4_K_M

# Configure for VVTV
sudo mkdir -p /vvtv/llm
sudo chown ollama:ollama /vvtv/llm
```

---

## 🌐 **CDN and Content Delivery**

### **Adult-Friendly CDN Options**

#### **Primary: AWS CloudFront**
- **Pros:** Native AWS integration, global edge locations
- **Cons:** Strict content policies, may require careful configuration
- **Configuration:** Use signed URLs, restrict to adult verification

#### **Alternative: BunnyCDN** 
- **Pros:** Adult-content friendly, excellent performance, lower cost
- **Cons:** External service, additional integration
- **Cost:** ~$0.01/GB vs CloudFront's ~$0.085/GB

#### **Recommended Hybrid Approach:**
```yaml
cdn_strategy:
  primary: "bunnycdn"  # Adult-friendly, cost-effective
  fallback: "cloudfront"  # AWS native, high reliability
  
  configuration:
    adult_verification: true
    geo_blocking: ["countries_where_restricted"]
    signed_urls: true
    cache_ttl: 
      segments: 3600  # 1 hour
      manifests: 60   # 1 minute
```

---

## 💾 **Storage Architecture**

### **Content Storage Strategy**
```yaml
storage_tiers:
  hot_storage:
    service: "EFS"
    mount: "/vvtv/storage/ready"
    performance: "General Purpose"
    cost: "$0.30/GB/month"
    use: "Active content (24-48h)"
    
  warm_storage:
    service: "S3 Standard"
    bucket: "vvtv-content-archive"
    cost: "$0.023/GB/month"
    use: "Recent content (1-30 days)"
    
  cold_storage:
    service: "S3 Glacier Instant Retrieval"
    bucket: "vvtv-content-cold"
    cost: "$0.004/GB/month"
    use: "Archive content (>30 days)"

lifecycle_policy:
  - hot_to_warm: 2_days
  - warm_to_cold: 30_days
  - delete_after: 365_days  # Compliance requirement
```

### **Database Strategy**
```yaml
databases:
  primary:
    service: "RDS PostgreSQL"
    instance: "db.t3.medium"
    storage: "100GB GP3"
    backup: "7 days"
    cost: "~$100/month"
    
  cache:
    service: "ElastiCache Redis"
    instance: "cache.t3.micro"
    cost: "~$15/month"
    use: "LLM response cache, session data"
```

---

## 🔒 **Security and Compliance**

### **Adult Content Compliance**
```yaml
compliance_measures:
  age_verification:
    method: "signed_tokens"
    provider: "custom_implementation"
    
  geo_restrictions:
    blocked_countries: ["countries_with_restrictions"]
    method: "cloudfront_geo_blocking"
    
  content_labeling:
    adult_warning: true
    content_rating: "18+"
    metadata_tags: ["adult", "explicit"]
    
  privacy:
    no_external_llm: true
    local_processing_only: true
    gdpr_compliant: true
    data_retention: "30_days_max"
```

### **Network Security**
```yaml
security_groups:
  streaming_node:
    inbound:
      - port: 22, source: "office_ip", protocol: "SSH"
      - port: 1935, source: "0.0.0.0/0", protocol: "RTMP"
      - port: 8080, source: "cdn_ips", protocol: "HLS"
    outbound:
      - port: 443, dest: "0.0.0.0/0", protocol: "HTTPS"
      
  llm_node:
    inbound:
      - port: 22, source: "office_ip", protocol: "SSH"
      - port: 11434, source: "streaming_sg", protocol: "Ollama"
    outbound:
      - port: 443, dest: "huggingface.co", protocol: "Model Downloads"
```

---

## 💰 **Cost Analysis**

### **Monthly Cost Breakdown**
```yaml
compute:
  primary_node: "$250"      # c6i.2xlarge
  llm_node: "$400"          # g5.xlarge  
  backup_node: "$50"        # c6i.large (stopped most time)
  
storage:
  efs_hot: "$150"           # 500GB active content
  s3_warm: "$50"            # 2TB recent content
  s3_cold: "$20"            # 5TB archive content
  
networking:
  data_transfer: "$100"     # 10TB/month outbound
  cdn_bunny: "$100"         # 10TB/month CDN
  
databases:
  rds_postgres: "$100"      # db.t3.medium
  elasticache: "$15"        # cache.t3.micro
  
total_monthly: "$1,235"     # ~$15k/year
```

### **Cost Optimization Strategies**
1. **Spot Instances:** Use for non-critical workloads (30-70% savings)
2. **Reserved Instances:** 1-year commitment (30-40% savings)
3. **S3 Lifecycle:** Automatic tiering saves 60-80% on storage
4. **CDN Optimization:** BunnyCDN vs CloudFront saves 80%

---

## 🚀 **Deployment Steps**

### **Phase 1: Infrastructure Setup (Week 1)**
```bash
# 1. Create VPC and networking
aws cloudformation create-stack --stack-name vvtv-network \
  --template-body file://cloudformation/network.yaml

# 2. Launch EC2 instances
aws ec2 run-instances --image-id ami-0abcdef1234567890 \
  --instance-type c6i.2xlarge --key-name vvtv-key \
  --security-group-ids sg-streaming --subnet-id subnet-primary

# 3. Setup EFS storage
aws efs create-file-system --creation-token vvtv-content-$(date +%s)

# 4. Configure RDS database
aws rds create-db-instance --db-instance-identifier vvtv-primary \
  --db-instance-class db.t3.medium --engine postgres
```

### **Phase 2: LLM Setup (Week 1)**
```bash
# 1. Launch GPU instance
aws ec2 run-instances --image-id ami-dlami-ubuntu \
  --instance-type g5.xlarge --key-name vvtv-key

# 2. Install Ollama and models
ssh -i vvtv-key.pem ubuntu@llm-instance
./install-ollama.sh

# 3. Configure VVTV LLM integration
scp configs/llm_local.toml ubuntu@llm-instance:/vvtv/configs/
```

### **Phase 3: Application Deployment (Week 2)**
```bash
# 1. Deploy VVTV application
git clone https://github.com/voulezvous-tv/voulezvous-tv-Rust.git
cd voulezvous-tv-Rust
cargo build --release

# 2. Configure for AWS
cp configs/aws_production.toml /vvtv/system/vvtv.toml

# 3. Setup systemd services
sudo systemctl enable vvtv-streaming
sudo systemctl enable vvtv-llm-pool
sudo systemctl start vvtv-streaming
```

### **Phase 4: CDN and Monitoring (Week 2)**
```bash
# 1. Configure BunnyCDN
curl -X POST "https://api.bunny.net/pullzone" \
  -H "AccessKey: YOUR_API_KEY" \
  -d '{"Name":"vvtv-streaming","OriginUrl":"https://origin.vvtv.aws"}'

# 2. Setup CloudWatch monitoring
aws logs create-log-group --log-group-name /vvtv/streaming
aws cloudwatch put-metric-alarm --alarm-name vvtv-cpu-high

# 3. Deploy Grafana dashboard
kubectl apply -f k8s/grafana-dashboard.yaml
```

---

## 🔧 **Configuration Files**

### **AWS-Optimized VVTV Config**
```toml
# /vvtv/system/vvtv.toml - AWS Production
[system]
node_name = "vvtv-aws-primary"
node_role = "broadcast"
environment = "aws_production"

[paths]
base_dir = "/vvtv"
data_dir = "/vvtv/data"
storage_dir = "/mnt/efs/storage"  # EFS mount
broadcast_dir = "/vvtv/broadcast"

[network]
rtmp_port = 1935
hls_port = 8080
llm_endpoint = "http://10.0.1.100:11434"  # Local LLM node

[aws]
region = "us-east-1"
s3_bucket = "vvtv-content-archive"
cloudwatch_enabled = true
efs_mount = "/mnt/efs"

[cdn]
primary = "bunnycdn"
bunny_api_key_path = "/vvtv/vault/keys/bunny_api.key"
bunny_zone_id = "your_zone_id"
origin_url = "https://origin.vvtv.aws"

[llm]
provider = "local_ollama"
endpoint = "http://10.0.1.100:11434"
models = ["llama3.1:8b-instruct-q4_K_M", "phi3.5:mini-instruct-q4_K_M"]
timeout_seconds = 30
max_concurrent = 3
```

### **Local LLM Configuration**
```yaml
# /vvtv/configs/llm_local.yaml
llm_config:
  provider: "ollama"
  endpoint: "http://localhost:11434"
  
  models:
    content_analysis: "llama3.1:8b-instruct-q4_K_M"
    query_enhancement: "phi3.5:mini-instruct-q4_K_M"
    
  privacy:
    external_calls: false
    logging_level: "minimal"
    content_filtering: false  # We handle adult content appropriately
    
  performance:
    gpu_memory_fraction: 0.8
    cpu_threads: 4
    batch_size: 1
    
  adult_content:
    analysis_enabled: true
    content_warnings: true
    compliance_logging: true
```

---

## 📊 **Monitoring and Alerting**

### **CloudWatch Metrics**
```yaml
custom_metrics:
  - name: "VVTV.StreamHealth"
    namespace: "VVTV/Streaming"
    dimensions: ["NodeId", "Region"]
    
  - name: "VVTV.LLMLatency" 
    namespace: "VVTV/AI"
    dimensions: ["Model", "Task"]
    
  - name: "VVTV.ContentBuffer"
    namespace: "VVTV/Content"
    dimensions: ["BufferType"]

alarms:
  - name: "VVTV-StreamDown"
    metric: "VVTV.StreamHealth"
    threshold: 0
    comparison: "LessThanThreshold"
    
  - name: "VVTV-HighLLMLatency"
    metric: "VVTV.LLMLatency"
    threshold: 5000  # 5 seconds
    comparison: "GreaterThanThreshold"
```

---

## 🎯 **Success Criteria**

### **Technical KPIs**
- **Stream Uptime:** >99.9%
- **LLM Response Time:** <2 seconds P95
- **Content Buffer:** >4 hours maintained
- **CDN Cache Hit Rate:** >90%

### **Business KPIs**
- **Monthly Cost:** <$1,500
- **Content Compliance:** 100% (no violations)
- **Privacy Compliance:** 100% (local processing only)
- **Viewer Experience:** <3 second startup time

---

## ⚠️ **Risk Mitigation**

### **Adult Content Risks**
1. **Content Policy Violations**
   - **Mitigation:** Use adult-friendly CDN (BunnyCDN)
   - **Backup:** Self-hosted CDN on AWS

2. **Payment Processing**
   - **Risk:** Adult content payment restrictions
   - **Mitigation:** Use adult-friendly payment processors

3. **Legal Compliance**
   - **Risk:** Varying international laws
   - **Mitigation:** Geo-blocking, age verification

### **Technical Risks**
1. **LLM Node Failure**
   - **Mitigation:** Fallback to deterministic mode
   - **Recovery:** Auto-restart, backup models

2. **AWS Service Limits**
   - **Risk:** EC2 instance limits, bandwidth caps
   - **Mitigation:** Reserved capacity, multi-region

---

## 🚀 **Next Steps**

1. **Immediate (This Week)**
   - Set up AWS account with adult content approval
   - Configure BunnyCDN account
   - Prepare deployment scripts

2. **Short Term (2 Weeks)**
   - Deploy infrastructure
   - Install and configure local LLM
   - Test streaming pipeline

3. **Medium Term (1 Month)**
   - Full production deployment
   - Monitoring and alerting setup
   - Performance optimization

4. **Long Term (3 Months)**
   - Multi-region deployment
   - Advanced analytics
   - Cost optimization

---

**This deployment plan provides a robust, compliant, and cost-effective solution for running VVTV on AWS while maintaining privacy through local LLM processing and ensuring adult content compliance.**
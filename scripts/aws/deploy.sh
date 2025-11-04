#!/usr/bin/env bash
# VVTV AWS Deployment Script
# Deploys VVTV streaming platform on AWS with local LLM

set -euo pipefail

# Configuration
AWS_REGION="us-east-1"
VPC_CIDR="10.0.0.0/16"
SUBNET_PUBLIC_CIDR="10.0.1.0/24"
SUBNET_PRIVATE_CIDR="10.0.2.0/24"
KEY_NAME="vvtv-aws-key"
STACK_NAME="vvtv-production"

# Instance types
STREAMING_INSTANCE="c6i.2xlarge"
LLM_INSTANCE="g5.xlarge"
BACKUP_INSTANCE="c6i.large"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[DEPLOY]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[DEPLOY WARN]${NC} $1"
}

error() {
    echo -e "${RED}[DEPLOY ERROR]${NC} $1"
    exit 1
}

info() {
    echo -e "${BLUE}[DEPLOY INFO]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check AWS CLI
    if ! command -v aws &> /dev/null; then
        error "AWS CLI not found. Please install AWS CLI."
    fi
    
    # Check AWS credentials
    if ! aws sts get-caller-identity &> /dev/null; then
        error "AWS credentials not configured. Run 'aws configure'."
    fi
    
    # Check jq
    if ! command -v jq &> /dev/null; then
        error "jq not found. Please install jq for JSON parsing."
    fi
    
    # Check region
    aws configure set region "$AWS_REGION"
    
    log "Prerequisites check passed ✓"
}

# Create VPC and networking
create_network() {
    log "Creating VPC and networking infrastructure..."
    
    # Create VPC
    VPC_ID=$(aws ec2 create-vpc \
        --cidr-block "$VPC_CIDR" \
        --tag-specifications "ResourceType=vpc,Tags=[{Key=Name,Value=vvtv-vpc}]" \
        --query 'Vpc.VpcId' --output text)
    
    info "Created VPC: $VPC_ID"
    
    # Enable DNS hostnames
    aws ec2 modify-vpc-attribute --vpc-id "$VPC_ID" --enable-dns-hostnames
    
    # Create Internet Gateway
    IGW_ID=$(aws ec2 create-internet-gateway \
        --tag-specifications "ResourceType=internet-gateway,Tags=[{Key=Name,Value=vvtv-igw}]" \
        --query 'InternetGateway.InternetGatewayId' --output text)
    
    aws ec2 attach-internet-gateway --vpc-id "$VPC_ID" --internet-gateway-id "$IGW_ID"
    
    # Create public subnet
    PUBLIC_SUBNET_ID=$(aws ec2 create-subnet \
        --vpc-id "$VPC_ID" \
        --cidr-block "$SUBNET_PUBLIC_CIDR" \
        --availability-zone "${AWS_REGION}a" \
        --tag-specifications "ResourceType=subnet,Tags=[{Key=Name,Value=vvtv-public-subnet}]" \
        --query 'Subnet.SubnetId' --output text)
    
    # Create private subnet
    PRIVATE_SUBNET_ID=$(aws ec2 create-subnet \
        --vpc-id "$VPC_ID" \
        --cidr-block "$SUBNET_PRIVATE_CIDR" \
        --availability-zone "${AWS_REGION}a" \
        --tag-specifications "ResourceType=subnet,Tags=[{Key=Name,Value=vvtv-private-subnet}]" \
        --query 'Subnet.SubnetId' --output text)
    
    # Create NAT Gateway for private subnet
    ALLOCATION_ID=$(aws ec2 allocate-address --domain vpc --query 'AllocationId' --output text)
    NAT_GW_ID=$(aws ec2 create-nat-gateway \
        --subnet-id "$PUBLIC_SUBNET_ID" \
        --allocation-id "$ALLOCATION_ID" \
        --tag-specifications "ResourceType=nat-gateway,Tags=[{Key=Name,Value=vvtv-nat-gw}]" \
        --query 'NatGateway.NatGatewayId' --output text)
    
    # Wait for NAT Gateway to be available
    log "Waiting for NAT Gateway to be available..."
    aws ec2 wait nat-gateway-available --nat-gateway-ids "$NAT_GW_ID"
    
    # Create route tables
    PUBLIC_RT_ID=$(aws ec2 create-route-table \
        --vpc-id "$VPC_ID" \
        --tag-specifications "ResourceType=route-table,Tags=[{Key=Name,Value=vvtv-public-rt}]" \
        --query 'RouteTable.RouteTableId' --output text)
    
    PRIVATE_RT_ID=$(aws ec2 create-route-table \
        --vpc-id "$VPC_ID" \
        --tag-specifications "ResourceType=route-table,Tags=[{Key=Name,Value=vvtv-private-rt}]" \
        --query 'RouteTable.RouteTableId' --output text)
    
    # Add routes
    aws ec2 create-route --route-table-id "$PUBLIC_RT_ID" --destination-cidr-block 0.0.0.0/0 --gateway-id "$IGW_ID"
    aws ec2 create-route --route-table-id "$PRIVATE_RT_ID" --destination-cidr-block 0.0.0.0/0 --nat-gateway-id "$NAT_GW_ID"
    
    # Associate subnets with route tables
    aws ec2 associate-route-table --subnet-id "$PUBLIC_SUBNET_ID" --route-table-id "$PUBLIC_RT_ID"
    aws ec2 associate-route-table --subnet-id "$PRIVATE_SUBNET_ID" --route-table-id "$PRIVATE_RT_ID"
    
    log "Network infrastructure created ✓"
}

# Create security groups
create_security_groups() {
    log "Creating security groups..."
    
    # Streaming node security group
    STREAMING_SG_ID=$(aws ec2 create-security-group \
        --group-name vvtv-streaming-sg \
        --description "VVTV Streaming Node Security Group" \
        --vpc-id "$VPC_ID" \
        --tag-specifications "ResourceType=security-group,Tags=[{Key=Name,Value=vvtv-streaming-sg}]" \
        --query 'GroupId' --output text)
    
    # LLM node security group
    LLM_SG_ID=$(aws ec2 create-security-group \
        --group-name vvtv-llm-sg \
        --description "VVTV LLM Node Security Group" \
        --vpc-id "$VPC_ID" \
        --tag-specifications "ResourceType=security-group,Tags=[{Key=Name,Value=vvtv-llm-sg}]" \
        --query 'GroupId' --output text)
    
    # Get office IP for SSH access
    OFFICE_IP=$(curl -s https://checkip.amazonaws.com)/32
    
    # Streaming node rules
    aws ec2 authorize-security-group-ingress --group-id "$STREAMING_SG_ID" --protocol tcp --port 22 --cidr "$OFFICE_IP"
    aws ec2 authorize-security-group-ingress --group-id "$STREAMING_SG_ID" --protocol tcp --port 1935 --cidr 0.0.0.0/0  # RTMP
    aws ec2 authorize-security-group-ingress --group-id "$STREAMING_SG_ID" --protocol tcp --port 8080 --cidr 0.0.0.0/0  # HLS
    aws ec2 authorize-security-group-ingress --group-id "$STREAMING_SG_ID" --protocol tcp --port 9000 --cidr "$VPC_CIDR"  # Control
    
    # LLM node rules (private subnet)
    aws ec2 authorize-security-group-ingress --group-id "$LLM_SG_ID" --protocol tcp --port 22 --cidr "$OFFICE_IP"
    aws ec2 authorize-security-group-ingress --group-id "$LLM_SG_ID" --protocol tcp --port 11434 --source-group "$STREAMING_SG_ID"  # Ollama
    
    log "Security groups created ✓"
}

# Create EFS file system
create_efs() {
    log "Creating EFS file system for shared storage..."
    
    EFS_ID=$(aws efs create-file-system \
        --creation-token "vvtv-content-$(date +%s)" \
        --performance-mode generalPurpose \
        --throughput-mode provisioned \
        --provisioned-throughput-in-mibps 100 \
        --tags Key=Name,Value=vvtv-content-storage \
        --query 'FileSystemId' --output text)
    
    # Wait for EFS to be available
    log "Waiting for EFS to be available..."
    aws efs wait file-system-available --file-system-id "$EFS_ID"
    
    # Create mount targets
    aws efs create-mount-target \
        --file-system-id "$EFS_ID" \
        --subnet-id "$PUBLIC_SUBNET_ID" \
        --security-groups "$STREAMING_SG_ID"
    
    aws efs create-mount-target \
        --file-system-id "$EFS_ID" \
        --subnet-id "$PRIVATE_SUBNET_ID" \
        --security-groups "$LLM_SG_ID"
    
    info "Created EFS: $EFS_ID"
    log "EFS file system created ✓"
}

# Create S3 buckets
create_s3_buckets() {
    log "Creating S3 buckets for content storage..."
    
    # Content archive bucket
    aws s3 mb "s3://vvtv-content-archive-$(date +%s)" --region "$AWS_REGION"
    
    # Backup bucket
    aws s3 mb "s3://vvtv-system-backups-$(date +%s)" --region "$AWS_REGION"
    
    # Logs bucket
    aws s3 mb "s3://vvtv-logs-archive-$(date +%s)" --region "$AWS_REGION"
    
    log "S3 buckets created ✓"
}

# Launch streaming instance
launch_streaming_instance() {
    log "Launching streaming instance ($STREAMING_INSTANCE)..."
    
    # Get latest Amazon Linux 2023 AMI
    AMI_ID=$(aws ec2 describe-images \
        --owners amazon \
        --filters "Name=name,Values=al2023-ami-*" "Name=architecture,Values=x86_64" \
        --query 'Images | sort_by(@, &CreationDate) | [-1].ImageId' \
        --output text)
    
    # User data script for streaming node
    cat > /tmp/streaming-userdata.sh << 'EOF'
#!/bin/bash
yum update -y
yum install -y docker git htop iotop

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Install FFmpeg
yum install -y ffmpeg

# Install NGINX with RTMP module
amazon-linux-extras install -y nginx1
yum install -y nginx-mod-rtmp

# Mount EFS
yum install -y amazon-efs-utils
mkdir -p /mnt/efs
echo "EFS_ID.efs.AWS_REGION.amazonaws.com:/ /mnt/efs efs defaults,_netdev" >> /etc/fstab
mount -a

# Create VVTV directory structure
mkdir -p /vvtv/{data,cache,storage,broadcast,system,vault}
chown -R ec2-user:ec2-user /vvtv

# Install CloudWatch agent
wget https://s3.amazonaws.com/amazoncloudwatch-agent/amazon_linux/amd64/latest/amazon-cloudwatch-agent.rpm
rpm -U ./amazon-cloudwatch-agent.rpm
EOF
    
    # Replace placeholders
    sed -i "s/EFS_ID/$EFS_ID/g" /tmp/streaming-userdata.sh
    sed -i "s/AWS_REGION/$AWS_REGION/g" /tmp/streaming-userdata.sh
    
    STREAMING_INSTANCE_ID=$(aws ec2 run-instances \
        --image-id "$AMI_ID" \
        --instance-type "$STREAMING_INSTANCE" \
        --key-name "$KEY_NAME" \
        --security-group-ids "$STREAMING_SG_ID" \
        --subnet-id "$PUBLIC_SUBNET_ID" \
        --associate-public-ip-address \
        --user-data file:///tmp/streaming-userdata.sh \
        --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=vvtv-streaming-primary}]" \
        --query 'Instances[0].InstanceId' \
        --output text)
    
    info "Launched streaming instance: $STREAMING_INSTANCE_ID"
    log "Streaming instance launched ✓"
}

# Launch LLM instance
launch_llm_instance() {
    log "Launching LLM instance ($LLM_INSTANCE)..."
    
    # Get Deep Learning AMI
    DL_AMI_ID=$(aws ec2 describe-images \
        --owners amazon \
        --filters "Name=name,Values=Deep Learning AMI GPU PyTorch*Ubuntu*" \
        --query 'Images | sort_by(@, &CreationDate) | [-1].ImageId' \
        --output text)
    
    # User data script for LLM node
    cat > /tmp/llm-userdata.sh << 'EOF'
#!/bin/bash
apt-get update -y

# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh
systemctl enable ollama
systemctl start ollama

# Configure Ollama for network access
mkdir -p /etc/systemd/system/ollama.service.d
cat > /etc/systemd/system/ollama.service.d/override.conf << 'OLLAMA_EOF'
[Service]
Environment="OLLAMA_HOST=0.0.0.0:11434"
OLLAMA_EOF

systemctl daemon-reload
systemctl restart ollama

# Pull models (this will take a while)
ollama pull llama3.1:8b-instruct-q4_K_M
ollama pull phi3.5:mini-instruct-q4_K_M

# Create VVTV LLM directory
mkdir -p /vvtv/llm
chown -R ubuntu:ubuntu /vvtv

# Install monitoring tools
apt-get install -y htop nvidia-smi
EOF
    
    LLM_INSTANCE_ID=$(aws ec2 run-instances \
        --image-id "$DL_AMI_ID" \
        --instance-type "$LLM_INSTANCE" \
        --key-name "$KEY_NAME" \
        --security-group-ids "$LLM_SG_ID" \
        --subnet-id "$PRIVATE_SUBNET_ID" \
        --user-data file:///tmp/llm-userdata.sh \
        --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=vvtv-llm-node}]" \
        --query 'Instances[0].InstanceId' \
        --output text)
    
    info "Launched LLM instance: $LLM_INSTANCE_ID"
    log "LLM instance launched ✓"
}

# Wait for instances to be ready
wait_for_instances() {
    log "Waiting for instances to be ready..."
    
    aws ec2 wait instance-running --instance-ids "$STREAMING_INSTANCE_ID" "$LLM_INSTANCE_ID"
    
    # Wait additional time for user data scripts to complete
    log "Waiting for initialization scripts to complete (this may take 10-15 minutes)..."
    sleep 600  # 10 minutes for basic setup
    
    log "Instances should be ready ✓"
}

# Deploy VVTV application
deploy_application() {
    log "Deploying VVTV application..."
    
    # Get streaming instance public IP
    STREAMING_IP=$(aws ec2 describe-instances \
        --instance-ids "$STREAMING_INSTANCE_ID" \
        --query 'Reservations[0].Instances[0].PublicIpAddress' \
        --output text)
    
    # Get LLM instance private IP
    LLM_IP=$(aws ec2 describe-instances \
        --instance-ids "$LLM_INSTANCE_ID" \
        --query 'Reservations[0].Instances[0].PrivateIpAddress' \
        --output text)
    
    info "Streaming node IP: $STREAMING_IP"
    info "LLM node IP: $LLM_IP"
    
    # Update configuration with actual IPs
    sed -i "s/10.0.1.100/$LLM_IP/g" configs/aws_production.toml
    
    # Copy application files to streaming instance
    log "Copying application files..."
    scp -i "$KEY_NAME.pem" -o StrictHostKeyChecking=no \
        configs/aws_production.toml \
        "ec2-user@$STREAMING_IP:/tmp/vvtv.toml"
    
    # SSH and setup application
    ssh -i "$KEY_NAME.pem" -o StrictHostKeyChecking=no "ec2-user@$STREAMING_IP" << 'EOF'
# Clone VVTV repository
cd /home/ec2-user
git clone https://github.com/voulezvous-tv/voulezvous-tv-Rust.git
cd voulezvous-tv-Rust

# Build application
source ~/.cargo/env
cargo build --release

# Setup configuration
sudo mkdir -p /vvtv/system
sudo cp /tmp/vvtv.toml /vvtv/system/
sudo chown -R ec2-user:ec2-user /vvtv

# Create systemd service
sudo tee /etc/systemd/system/vvtv-streaming.service > /dev/null << 'SERVICE_EOF'
[Unit]
Description=VVTV Streaming Service
After=network.target

[Service]
Type=simple
User=ec2-user
WorkingDirectory=/home/ec2-user/voulezvous-tv-Rust
ExecStart=/home/ec2-user/voulezvous-tv-Rust/target/release/vvtv-streaming
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICE_EOF

sudo systemctl daemon-reload
sudo systemctl enable vvtv-streaming
EOF
    
    log "Application deployed ✓"
}

# Setup monitoring
setup_monitoring() {
    log "Setting up CloudWatch monitoring..."
    
    # Create CloudWatch log group
    aws logs create-log-group --log-group-name "/vvtv/application" --region "$AWS_REGION" || true
    
    # Create SNS topic for alerts
    SNS_TOPIC_ARN=$(aws sns create-topic --name vvtv-alerts --query 'TopicArn' --output text)
    
    # Create CloudWatch alarms
    aws cloudwatch put-metric-alarm \
        --alarm-name "VVTV-HighCPU" \
        --alarm-description "VVTV High CPU Usage" \
        --metric-name CPUUtilization \
        --namespace AWS/EC2 \
        --statistic Average \
        --period 300 \
        --threshold 80 \
        --comparison-operator GreaterThanThreshold \
        --dimensions Name=InstanceId,Value="$STREAMING_INSTANCE_ID" \
        --evaluation-periods 2 \
        --alarm-actions "$SNS_TOPIC_ARN"
    
    log "Monitoring setup complete ✓"
}

# Print deployment summary
print_summary() {
    log "🎉 VVTV AWS Deployment Complete!"
    echo
    info "Deployment Summary:"
    echo "  VPC ID: $VPC_ID"
    echo "  Streaming Instance: $STREAMING_INSTANCE_ID"
    echo "  LLM Instance: $LLM_INSTANCE_ID"
    echo "  EFS ID: $EFS_ID"
    echo "  SNS Topic: $SNS_TOPIC_ARN"
    echo
    info "Next Steps:"
    echo "  1. SSH to streaming instance: ssh -i $KEY_NAME.pem ec2-user@$STREAMING_IP"
    echo "  2. Start VVTV service: sudo systemctl start vvtv-streaming"
    echo "  3. Check logs: journalctl -u vvtv-streaming -f"
    echo "  4. Configure CDN (BunnyCDN) with origin: http://$STREAMING_IP:8080"
    echo "  5. Setup domain DNS to point to CDN"
    echo
    warn "Remember to:"
    echo "  - Configure adult content compliance settings"
    echo "  - Set up age verification"
    echo "  - Configure geo-blocking for restricted countries"
    echo "  - Test LLM connectivity between instances"
}

# Main deployment function
main() {
    log "Starting VVTV AWS deployment..."
    
    check_prerequisites
    create_network
    create_security_groups
    create_efs
    create_s3_buckets
    launch_streaming_instance
    launch_llm_instance
    wait_for_instances
    deploy_application
    setup_monitoring
    print_summary
    
    log "Deployment completed successfully! 🚀"
}

# Handle command line arguments
case "${1:-deploy}" in
    "deploy")
        main
        ;;
    "cleanup")
        log "Cleaning up AWS resources..."
        # Add cleanup logic here
        warn "Cleanup not implemented yet"
        ;;
    *)
        echo "Usage: $0 [deploy|cleanup]"
        exit 1
        ;;
esac
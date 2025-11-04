# VVTV Streaming Runbook

## Overview
This runbook covers operational procedures for the VVTV streaming and broadcast system.

## Quick Reference

| Alert | Severity | Response Time | Escalation |
|-------|----------|---------------|------------|
| segment_age > 10s | Critical | 5 minutes | Level 2 after 10min |
| encoder_drops > 0.5% | Major | 15 minutes | Level 2 after 30min |
| ingest_down | Critical | 2 minutes | Level 2 after 5min |
| bitrate_variance > 25% | Major | 15 minutes | Level 2 after 30min |

## Procedures

### 🚨 segment-age: High Segment Age

**Symptoms:** HLS segments are older than 10 seconds, causing high latency

**Impact:** Viewers experience increased latency, poor live experience

**Immediate Actions:**
1. Check encoder status:
   ```bash
   vvtvctl streaming status
   ps aux | grep ffmpeg
   ```

2. Check system resources:
   ```bash
   top -p $(pgrep ffmpeg)
   iostat -x 1 5
   df -h /vvtv/broadcast
   ```

3. If encoder is stuck/frozen:
   ```bash
   # Restart encoder without stopping NGINX
   vvtvctl streaming restart-encoder
   ```

4. If buffer is low (< 30min):
   ```bash
   # Inject emergency content
   vvtvctl streaming emergency --inject 5 --reason "segment_age_high"
   ```

5. Capture diagnostics:
   ```bash
   vvtvctl ops bundle --scope streaming --output /tmp/streaming_debug.tar.gz
   ```

**Timer:** 10 minutes to stabilize before escalating to Level 2

**Escalation:** If segment age doesn't improve within 10 minutes, escalate with diagnostic bundle

---

### 🚨 encoder-drops: High Frame Drop Rate

**Symptoms:** Video encoder is dropping frames (> 0.5%)

**Impact:** Video quality degradation, stuttering

**Immediate Actions:**
1. Check encoder performance:
   ```bash
   vvtvctl streaming stats --encoder
   ffprobe -v quiet -show_streams rtmp://localhost/live/main
   ```

2. Check CPU and memory:
   ```bash
   htop -p $(pgrep ffmpeg)
   free -h
   ```

3. Reduce encoder load temporarily:
   ```bash
   # Lower quality preset temporarily
   vvtvctl flags set encoder.preset=fast
   vvtvctl flags set encoder.crf=23  # Increase CRF for lower CPU usage
   ```

4. Check input source stability:
   ```bash
   # Verify queue has stable content
   vvtvctl queue summary
   ls -la /vvtv/storage/ready/ | tail -10
   ```

5. If drops persist, restart with lower settings:
   ```bash
   vvtvctl streaming restart-encoder --preset fast --bitrate 3000
   ```

**Recovery:** Monitor for 15 minutes, gradually restore quality settings

---

### 🚨 ingest-down: RTMP Ingest Unavailable

**Symptoms:** RTMP ingest endpoint not responding

**Impact:** Cannot receive video stream, broadcast stops

**Immediate Actions:**
1. Check NGINX-RTMP status:
   ```bash
   systemctl status nginx-rtmp
   # or on macOS:
   launchctl list | grep nginx
   ```

2. Test RTMP endpoint:
   ```bash
   ffprobe rtmp://localhost/live/main
   curl -I http://localhost:8080/hls/main.m3u8
   ```

3. Check NGINX configuration:
   ```bash
   nginx -t -c /vvtv/configs/nginx/rtmp.conf
   ```

4. Restart NGINX-RTMP if needed:
   ```bash
   systemctl restart nginx-rtmp
   # or on macOS:
   sudo launchctl unload /Library/LaunchDaemons/nginx-rtmp.plist
   sudo launchctl load /Library/LaunchDaemons/nginx-rtmp.plist
   ```

5. Verify encoder can reconnect:
   ```bash
   vvtvctl streaming test-ingest
   ```

**Timer:** 5 minutes maximum downtime before escalation

---

### 🚨 bitrate-variance: Unstable Stream Bitrate

**Symptoms:** Stream bitrate varying more than 25%

**Impact:** Inconsistent quality, potential buffering for viewers

**Immediate Actions:**
1. Check encoder settings:
   ```bash
   vvtvctl streaming config --show-encoder
   ```

2. Verify input content consistency:
   ```bash
   # Check if queue has mixed quality content
   vvtvctl queue show --format json | jq '.items[] | {id, resolution, bitrate}'
   ```

3. Enable bitrate smoothing:
   ```bash
   vvtvctl flags set encoder.rate_control=cbr
   vvtvctl flags set encoder.buffer_size=4000k
   ```

4. Check network stability:
   ```bash
   ping -c 10 8.8.8.8
   iftop -i $(route | grep default | awk '{print $8}')
   ```

5. If variance continues, use fixed bitrate mode:
   ```bash
   vvtvctl streaming restart-encoder --mode cbr --bitrate 4500
   ```

**Recovery:** Monitor bitrate stability for 30 minutes

---

## Diagnostic Commands

### System Health
```bash
# Overall streaming health
vvtvctl streaming health

# Detailed encoder stats
vvtvctl streaming stats --detailed

# Check segment timing
ls -la /vvtv/broadcast/hls/ | tail -20

# Monitor real-time metrics
watch -n 2 'vvtvctl streaming metrics --live'
```

### Performance Analysis
```bash
# CPU usage by component
top -p $(pgrep -d, -f "ffmpeg|nginx")

# I/O performance
iostat -x 1 10

# Network throughput
iftop -t -s 60

# Memory usage
free -h && echo "---" && ps aux --sort=-%mem | head -10
```

### Log Analysis
```bash
# Recent encoder errors
tail -100 /vvtv/logs/streaming.log | grep ERROR

# NGINX-RTMP logs
tail -100 /var/log/nginx/rtmp_access.log

# System messages
journalctl -u nginx-rtmp -f --since "10 minutes ago"
```

## Preventive Maintenance

### Daily Checks
- [ ] Verify segment freshness < 6s
- [ ] Check encoder drop rate < 0.1%
- [ ] Confirm bitrate stability
- [ ] Review error logs

### Weekly Tasks
- [ ] Rotate NGINX logs
- [ ] Clean old HLS segments
- [ ] Update encoder presets if needed
- [ ] Test failover procedures

### Monthly Tasks
- [ ] Review SLO compliance
- [ ] Update runbook procedures
- [ ] Capacity planning review
- [ ] Disaster recovery test

## Escalation Contacts

| Level | Contact | Response Time | Scope |
|-------|---------|---------------|-------|
| L1 | SRE On-Call | 5 minutes | Immediate response |
| L2 | Streaming Lead | 15 minutes | Technical expertise |
| L3 | Engineering Manager | 30 minutes | Resource allocation |
| L4 | CTO | 1 hour | Business decisions |

## Related Documentation
- [Streaming Architecture](../docs/streaming_architecture.md)
- [Encoder Configuration](../configs/encoder_presets.md)
- [NGINX-RTMP Setup](../configs/nginx/rtmp.conf)
- [Monitoring Dashboards](../dashboards/streaming_health.json)
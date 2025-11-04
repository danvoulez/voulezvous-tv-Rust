#!/bin/bash
# VVTV Thermal and hardware inspection

set -euo pipefail

read_cpu_temp() {
    if command -v osx-cpu-temp >/dev/null 2>&1; then
        osx-cpu-temp -c | cut -d'°' -f1
    elif [[ -f /sys/class/thermal/thermal_zone0/temp ]]; then
        awk '{print $1/1000}' /sys/class/thermal/thermal_zone0/temp
    elif command -v sensors >/dev/null 2>&1; then
        sensors 2>/dev/null | awk '/^Package id 0:/ {gsub("°C", "", $4); print $4; exit}'
    fi
}

read_gpu_temp() {
    if command -v nvidia-smi >/dev/null 2>&1; then
        nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits 2>/dev/null | head -n1
    elif command -v sensors >/dev/null 2>&1; then
        sensors 2>/dev/null | awk '/GPU/ {gsub("°C", "", $2); print $2; exit}'
    fi
}

read_ssd_temp() {
    if command -v smartctl >/dev/null 2>&1; then
        smartctl -A ${SSD_DEVICE:-/dev/sda} 2>/dev/null |
            awk '/Temperature_Celsius/ {print $10; exit}'
    fi
}

read_ssd_wear() {
    if command -v smartctl >/dev/null 2>&1; then
        smartctl -A ${SSD_DEVICE:-/dev/sda} 2>/dev/null |
            awk '/Wear_Leveling_Count|Percent_Lifetime_Used/ {print $10; exit}'
    fi
}

read_fan_rpm() {
    if command -v ipmitool >/dev/null 2>&1; then
        ipmitool sdr list 2>/dev/null | awk '/FAN/ {gsub("RPM", "", $4); print $4; exit}'
    elif command -v powermetrics >/dev/null 2>&1; then
        powermetrics -n 1 --samplers smc 2>/dev/null |
            awk -F ':' '/Fan:/ {gsub("rpm", "", $2); print $2; exit}' |
            tr -d ' '
    fi
}

json_number() {
    local value=$1
    if [[ -z "$value" ]]; then
        echo "null"
    else
        printf '%s' "$value"
    fi
}

cpu_temp=$(read_cpu_temp)
gpu_temp=$(read_gpu_temp)
ssd_temp=$(read_ssd_temp)
ssd_wear=$(read_ssd_wear)
fan_rpm=$(read_fan_rpm)

cat <<JSON
{
  "cpu_temp_c": $(json_number "$cpu_temp"),
  "gpu_temp_c": $(json_number "$gpu_temp"),
  "ssd_temp_c": $(json_number "$ssd_temp"),
  "ssd_wear_percent": $(json_number "$ssd_wear"),
  "fan_rpm": $(json_number "$fan_rpm")
}
JSON

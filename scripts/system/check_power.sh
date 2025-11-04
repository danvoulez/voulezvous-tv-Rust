#!/bin/bash
# VVTV Power and UPS inspection

set -euo pipefail

UPS_SOURCE=${UPS_SOURCE:-ups@localhost}

log_json() {
    local value=$1
    if [[ -z "$value" ]]; then
        echo "null"
    else
        printf '%s' "$value"
    fi
}

read_powermetrics() {
    if command -v powermetrics >/dev/null 2>&1; then
        powermetrics -n 1 --samplers smc 2>/dev/null |
            awk -F ':' '/CPU Power/ {gsub("W", "", $2); print $2; exit}' |
            tr -d ' '
    fi
}

read_upower() {
    if command -v upower >/dev/null 2>&1; then
        local device
        device=$(upower -e | grep -m1 BAT || true)
        if [[ -n "$device" ]]; then
            upower -i "$device" 2>/dev/null |
                awk -F ':' '/energy-rate/ {gsub("W", "", $2); print $2; exit}' |
                tr -d ' '
        fi
    fi
}

read_proc_power() {
    if [[ -f /sys/class/powercap/intel-rapl:0/energy_uj ]]; then
        local energy1 energy2
        energy1=$(cat /sys/class/powercap/intel-rapl:0/energy_uj)
        sleep 1
        energy2=$(cat /sys/class/powercap/intel-rapl:0/energy_uj)
        awk -v e1="$energy1" -v e2="$energy2" 'BEGIN { printf "%.2f", (e2-e1)/1000000 }'
    fi
}

read_ups_field() {
    local field=$1
    if command -v upsc >/dev/null 2>&1; then
        upsc "$UPS_SOURCE" 2>/dev/null | awk -v f="$field" '$1==f {print $2}'
    fi
}

power_watts=$(read_powermetrics)
if [[ -z "$power_watts" ]]; then
    power_watts=$(read_upower)
fi
if [[ -z "$power_watts" ]]; then
    power_watts=$(read_proc_power)
fi

ups_runtime=$(read_ups_field "battery.runtime")
if [[ -n "$ups_runtime" ]]; then
    ups_runtime=$(awk -v v="$ups_runtime" 'BEGIN { printf "%.1f", v/60 }')
fi

ups_charge=$(read_ups_field "battery.charge")
ups_status=$(read_ups_field "ups.status")

cat <<JSON
{
  "power_watts": $(log_json "$power_watts"),
  "ups_runtime_minutes": $(log_json "$ups_runtime"),
  "ups_charge_percent": $(log_json "$ups_charge"),
  "ups_status": $(if [[ -n "$ups_status" ]]; then printf '"%s"' "$ups_status"; else echo null; fi)
}
JSON

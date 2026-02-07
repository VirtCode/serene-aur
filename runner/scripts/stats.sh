#!/bin/sh

cd /sys/fs/cgroup

# peak memory usage (bytes)
if [ -f "memory.peak" ]; then MEMORY_PEAK=$(cat memory.peak); else MEMORY_PEAK="null"; fi

# user cpu time (us)
if [ -f "cpu.stat" ]; then USER_USEC=$(cat cpu.stat | grep user_usec | sed 's/ /\n/g' | sed -n '2 p'); else USER_USEC="null"; fi
# system cpu time (us)
if [ -f "cpu.stat" ]; then SYSTEM_USEC=$(cat cpu.stat | grep system_usec | sed 's/ /\n/g' | sed -n '2 p'); else SYSTEM_USEC="null"; fi

# total bytes read/written (bytes)
if [ -f "io.stat" ]; then
    TOTAL_RBYTES=0
    TOTAL_WBYTES=0
    while IFS= read -r line ; do
        DEV_RBYTES=$(echo $line | sed 's/ /\n/g' | sed -n '2 p' | sed 's/=/\n/g' | sed -n '2 p')
        DEV_WBYTES=$(echo $line | sed 's/ /\n/g' | sed -n '3 p' | sed 's/=/\n/g' | sed -n '2 p')
        if [[ ! -z "${DEV_RBYTES}" ]]; then TOTAL_RBYTES=$(($TOTAL_RBYTES + $DEV_RBYTES)); fi
        if [[ ! -z "${DEV_WBYTES}" ]]; then TOTAL_WBYTES=$(($TOTAL_WBYTES + $DEV_WBYTES)); fi
    done <<< "$(cat io.stat)"
else
    TOTAL_RBYTES="null"
    TOTAL_WBYTES="null"
fi

echo "{ \"memory_bytes_peak\": ${MEMORY_PEAK}, \"cpu_user_us\": ${USER_USEC}, \"cpu_system_us\": ${SYSTEM_USEC}, \"io_total_bytes_read\": ${TOTAL_RBYTES}, \"io_total_bytes_written\": ${TOTAL_WBYTES} }"

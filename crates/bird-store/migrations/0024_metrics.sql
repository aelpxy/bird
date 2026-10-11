CREATE TABLE metric_samples (
    service_id TEXT NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    machine_id TEXT NOT NULL,
    resolution INTEGER NOT NULL,
    at INTEGER NOT NULL,
    cpu_millicores INTEGER NOT NULL,
    cpu_peak_millicores INTEGER NOT NULL,
    memory_bytes INTEGER NOT NULL,
    memory_peak_bytes INTEGER NOT NULL,
    net_rx_bytes INTEGER NOT NULL,
    net_tx_bytes INTEGER NOT NULL,
    PRIMARY KEY (service_id, resolution, at, machine_id)
) WITHOUT ROWID;

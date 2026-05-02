(() => {
    setupSyncButton();
    setupCharts();

    function setupSyncButton() {
        const btn = document.getElementById("sync-btn");
        const progress = document.getElementById("sync-progress");
        if (!btn || !progress) return;

        let polling = null;

        async function poll() {
            try {
                const r = await fetch("/api/collection/sync/status");
                if (!r.ok) throw new Error("status " + r.status);
                const j = await r.json();
                if (j.status === "running") {
                    progress.textContent = "Syncing " + j.currentItem + " / " + j.totalItems;
                } else if (j.status === "idle") {
                    progress.textContent = "Sync complete.";
                    clearInterval(polling);
                    polling = null;
                    btn.disabled = false;
                    setTimeout(() => location.reload(), 1200);
                } else if (j.status === "error") {
                    progress.textContent = "Sync failed: " + (j.lastError || "unknown");
                    clearInterval(polling);
                    polling = null;
                    btn.disabled = false;
                }
            } catch (e) {
                progress.textContent = "Status check failed: " + e.message;
            }
        }

        btn.addEventListener("click", async () => {
            btn.disabled = true;
            progress.textContent = "Starting sync...";
            try {
                const r = await fetch("/api/collection/sync");
                if (!r.ok) {
                    const j = await r.json().catch(() => ({}));
                    throw new Error(j.error || r.statusText);
                }
            } catch (e) {
                progress.textContent = "Failed to start: " + e.message;
                btn.disabled = false;
                return;
            }
            polling = setInterval(poll, 3000);
            poll();
        });
    }

    async function setupCharts() {
        if (typeof ApexCharts === "undefined") return;
        if (!document.getElementById("value-chart")) return;

        let stats;
        try {
            const r = await fetch("/api/dashboard-stats");
            if (!r.ok) throw new Error("stats " + r.status);
            stats = await r.json();
        } catch (e) {
            console.error("dashboard-stats fetch failed", e);
            return;
        }

        renderValueChart(stats.valueHistory || []);
        renderCountChart(stats.itemCountHistory || []);
        renderYearChart(stats.yearDistribution || {});
    }

    function baseOptions(extra) {
        return Object.assign(
            {
                chart: {
                    background: "transparent",
                    foreColor: "#a3a3a3",
                    toolbar: { show: false },
                    animations: { enabled: false },
                },
                theme: { mode: "dark" },
                grid: { borderColor: "#262626", strokeDashArray: 3 },
                tooltip: { theme: "dark" },
                legend: { labels: { colors: "#d4d4d4" } },
            },
            extra,
        );
    }

    function renderValueChart(history) {
        const el = document.getElementById("value-chart");
        if (!el) return;
        const series = [
            {
                name: "Min",
                data: history
                    .filter((p) => p.min != null)
                    .map((p) => [Date.parse(p.timestamp), p.min]),
            },
            {
                name: "Mean",
                data: history
                    .filter((p) => p.mean != null)
                    .map((p) => [Date.parse(p.timestamp), p.mean]),
            },
            {
                name: "Max",
                data: history
                    .filter((p) => p.max != null)
                    .map((p) => [Date.parse(p.timestamp), p.max]),
            },
        ];
        const options = baseOptions({
            chart: {
                type: "line",
                height: 300,
                background: "transparent",
                foreColor: "#a3a3a3",
                toolbar: { show: false },
                animations: { enabled: false },
            },
            series,
            title: {
                text: "Collection value",
                style: { color: "#fafafa", fontSize: "14px" },
            },
            stroke: { curve: "smooth", width: 2 },
            colors: ["#3b82f6", "#a855f7", "#22c55e"],
            xaxis: { type: "datetime", labels: { datetimeUTC: false } },
            yaxis: {
                labels: { formatter: (v) => "$" + v.toFixed(0) },
            },
        });
        new ApexCharts(el, options).render();
    }

    function renderCountChart(history) {
        const el = document.getElementById("count-chart");
        if (!el) return;
        if (history.length < 2) {
            el.innerHTML =
                '<div class="text-neutral-500 text-sm p-8 text-center">Item count history will appear after a second sync snapshot.</div>';
            return;
        }
        const options = baseOptions({
            chart: {
                type: "area",
                height: 300,
                background: "transparent",
                foreColor: "#a3a3a3",
                toolbar: { show: false },
                animations: { enabled: false },
            },
            series: [
                {
                    name: "Items",
                    data: history.map((p) => [Date.parse(p.timestamp), p.count]),
                },
            ],
            title: {
                text: "Item count",
                style: { color: "#fafafa", fontSize: "14px" },
            },
            stroke: { curve: "smooth", width: 2 },
            colors: ["#3b82f6"],
            dataLabels: { enabled: false },
            fill: {
                type: "gradient",
                gradient: { opacityFrom: 0.4, opacityTo: 0.05 },
            },
            xaxis: { type: "datetime", labels: { datetimeUTC: false } },
            yaxis: { labels: { formatter: (v) => v.toFixed(0) } },
        });
        new ApexCharts(el, options).render();
    }

    function renderYearChart(dist) {
        const el = document.getElementById("year-chart");
        if (!el) return;
        // OrderedDist serializes as a JSON object. JS re-orders integer-like keys
        // ascending, which would destroy the backend's sort-by-count-desc intent,
        // so we re-sort here before truncating the long tail into "Other".
        const entries = Object.entries(dist).sort(([, a], [, b]) => b - a);
        const limit = 10;
        const labels = [];
        const series = [];
        let other = 0;
        entries.forEach(([label, count], i) => {
            if (i < limit) {
                labels.push(String(label));
                series.push(count);
            } else {
                other += count;
            }
        });
        if (other > 0) {
            labels.push("Other");
            series.push(other);
        }
        if (series.length === 0) {
            el.innerHTML =
                '<div class="text-neutral-500 text-sm p-8 text-center">No year data yet.</div>';
            return;
        }
        const options = baseOptions({
            chart: {
                type: "donut",
                height: 320,
                background: "transparent",
                foreColor: "#a3a3a3",
                toolbar: { show: false },
                animations: { enabled: false },
            },
            series,
            labels,
            colors: [
                "#3b82f6",
                "#a855f7",
                "#10b981",
                "#ec4899",
                "#f97316",
                "#06b6d4",
                "#f59e0b",
                "#6366f1",
                "#84cc16",
                "#d946ef",
                "#737373",
            ],
            stroke: { colors: ["#171717"] },
            legend: { position: "right", labels: { colors: "#d4d4d4" } },
            dataLabels: { enabled: false },
            plotOptions: {
                pie: {
                    donut: {
                        labels: {
                            show: true,
                            total: {
                                show: true,
                                label: "Total",
                                color: "#a3a3a3",
                                formatter: (w) =>
                                    w.globals.seriesTotals.reduce((a, b) => a + b, 0).toString(),
                            },
                        },
                    },
                },
            },
        });
        new ApexCharts(el, options).render();
    }
})();

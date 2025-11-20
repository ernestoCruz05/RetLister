let SERVER_URL = localStorage.getItem("retlister_server_url") || "http://localhost:8000";

export function setServerUrl(url) {
  SERVER_URL = url.replace(/\/$/, "");
  localStorage.setItem("retlister_server_url", SERVER_URL);
}

export function getServerUrl() {
  return SERVER_URL;
}

export async function listRestos() {
  const res = await fetch(`${SERVER_URL}/list`);
  if (!res.ok) throw new Error(`List failed: ${res.status}`);
  return res.json();
}

export async function addResto(payload) {
  const res = await fetch(`${SERVER_URL}/add`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(`Add failed: ${res.status}`);
  return res.json();
}

export async function removeResto(id) {
  const res = await fetch(`${SERVER_URL}/remove/${id}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`Remove failed: ${res.status}`);
}

export async function searchResto(params) {
  const q = new URLSearchParams(params).toString();
  const res = await fetch(`${SERVER_URL}/search?${q}`);
  if (!res.ok) throw new Error(`Search failed: ${res.status}`);
  return res.json();
}

export async function updateResto(id, payload) {
  const res = await fetch(`${SERVER_URL}/update/${id}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(`Update failed: ${res.status}`);
}

export async function getStats() {
  const res = await fetch(`${SERVER_URL}/stats`);
  if (!res.ok) throw new Error(`Stats failed: ${res.status}`);
  return res.json();
}

// ===== VAN API =====

export async function listVans() {
  const res = await fetch(`${SERVER_URL}/vans`);
  if (!res.ok) throw new Error(`List vans failed: ${res.status}`);
  return res.json();
}

export async function getVan(id) {
  const res = await fetch(`${SERVER_URL}/vans/${id}`);
  if (!res.ok) throw new Error(`Get van failed: ${res.status}`);
  return res.json();
}

export async function addVan(payload) {
  console.log('Adding van with payload:', payload);
  const res = await fetch(`${SERVER_URL}/vans`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const error = await res.json();
    console.error('Add van error:', error);
    throw new Error(error.error || `Add van failed: ${res.status}`);
  }
  return res.json();
}

export async function updateVan(id, payload) {
  const res = await fetch(`${SERVER_URL}/vans/${id}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const error = await res.json();
    throw new Error(error.error || `Update van failed: ${res.status}`);
  }
}

export async function deleteVan(id) {
  const res = await fetch(`${SERVER_URL}/vans/${id}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`Delete van failed: ${res.status}`);
}

// ===== OPTIMIZE API =====

export async function optimizeLoading(van_id, items) {
  const res = await fetch(`${SERVER_URL}/optimize`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ van_id, items }),
  });
  if (!res.ok) {
    const error = await res.json();
    throw new Error(error.error || `Optimize failed: ${res.status}`);
  }
  return res.json();
}

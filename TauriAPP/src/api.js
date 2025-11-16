const SERVER_URL = "http://localhost:8000";

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

#!/usr/bin/env bun
/**
 * Seed script: inserts simulated ramp_events + chat_messages into the debug DB.
 * Run: bun run scripts/seed-test-data.ts
 */
import { Database } from "bun:sqlite";
import { resolve } from "path";

const DB_PATH = resolve(import.meta.dir, "../src-tauri/target/debug/ampel.db");

const db = new Database(DB_PATH);
db.exec("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;");

// ── helpers ──────────────────────────────────────────────────────────────────

function ts(date: Date): string {
  return date.toISOString().replace("T", "T").slice(0, 19);
}

function hoursAgo(h: number, minutesOffset = 0): Date {
  const d = new Date("2026-05-08T00:00:00");
  d.setHours(h, minutesOffset, 0, 0);
  return d;
}

function yesterday(h: number, min = 0): Date {
  const d = new Date("2026-05-07T00:00:00");
  d.setHours(h, min, 0, 0);
  return d;
}

const USERS = ["C.Ziesch", "K.Müller", "H.Schmidt", "M.Wagner", "S.Fischer", "T.Bauer"];
const KENNZEICHEN = [
  "B-AB 1234", "M-XY 5678", "HH-CD 9012", "K-EF 3456", "F-GH 7890",
  "S-IJ 2345", "D-KL 6789", "L-MN 0123", "DD-OP 4567", "N-QR 8901",
  "BO-ST 3210", "MA-UV 7654", "AC-WX 1098", "KA-YZ 5432", "DO-AA 9876",
];
const FIRMEN = ["Rewe Logistik", "DHL Express", "DB Schenker", "Rhenus", "Dachser", "Kühne+Nagel"];

function rndUser() { return USERS[Math.floor(Math.random() * USERS.length)]; }
function rndKz()   { return KENNZEICHEN[Math.floor(Math.random() * KENNZEICHEN.length)]; }
function rndFirma(){ return FIRMEN[Math.floor(Math.random() * FIRMEN.length)]; }
function rndRamp() { return 30 + Math.floor(Math.random() * 28); } // 30–57

// ── clear old simulated data ──────────────────────────────────────────────────

db.exec(`
  DELETE FROM ramp_events   WHERE user IN ('C.Ziesch','K.Müller','H.Schmidt','M.Wagner','S.Fischer','T.Bauer');
  DELETE FROM chat_messages WHERE user IN ('C.Ziesch','K.Müller','H.Schmidt','M.Wagner','S.Fischer','T.Bauer');
`);

// ── ramp_events ───────────────────────────────────────────────────────────────

const insertEvent = db.prepare(`
  INSERT INTO ramp_events (timestamp, ramp_id, from_status, to_status, user, kennzeichen, reserviert_fuer, duration_min)
  VALUES (?, ?, ?, ?, ?, ?, ?, ?)
`);

type EventRow = [string, number, string, string, string, string | null, string | null, number | null];

const events: EventRow[] = [
  // ── Yesterday (2026-05-07) ─────────────────────────────────────────────────
  [ts(yesterday(6, 15)), 42, "free",    "closed",  "H.Schmidt",  "HH-CD 9012", null,              null],
  [ts(yesterday(6, 45)), 42, "closed",  "free",    "H.Schmidt",  null,         null,              30],
  [ts(yesterday(7,  0)), 35, "free",    "closed",  "K.Müller",   "M-XY 5678",  "Rewe Logistik",   null],
  [ts(yesterday(7, 30)), 38, "free",    "closed",  "M.Wagner",   "K-EF 3456",  null,              null],
  [ts(yesterday(8,  0)), 35, "closed",  "free",    "K.Müller",   null,         null,              60],
  [ts(yesterday(8, 15)), 31, "free",    "pending", "S.Fischer",  "B-AB 1234",  "DHL Express",     null],
  [ts(yesterday(8, 45)), 31, "pending", "closed",  "S.Fischer",  "B-AB 1234",  "DHL Express",     30],
  [ts(yesterday(9, 10)), 38, "closed",  "free",    "M.Wagner",   null,         null,              100],
  [ts(yesterday(9, 20)), 43, "free",    "closed",  "C.Ziesch",   "S-IJ 2345",  "DB Schenker",     null],
  [ts(yesterday(9, 50)), 46, "free",    "closed",  "T.Bauer",    "D-KL 6789",  null,              null],
  [ts(yesterday(10, 5)), 31, "closed",  "free",    "S.Fischer",  null,         null,              80],
  [ts(yesterday(10,30)), 43, "closed",  "free",    "C.Ziesch",   null,         null,              70],
  [ts(yesterday(10,45)), 50, "free",    "closed",  "K.Müller",   "F-GH 7890",  "Rhenus",          null],
  [ts(yesterday(11,15)), 46, "closed",  "free",    "T.Bauer",    null,         null,              85],
  [ts(yesterday(11,30)), 53, "free",    "closed",  "H.Schmidt",  "L-MN 0123",  null,              null],
  [ts(yesterday(12, 0)), 50, "closed",  "free",    "K.Müller",   null,         null,              75],
  [ts(yesterday(12,20)), 37, "free",    "pending", "S.Fischer",  "N-QR 8901",  "Dachser",         null],
  [ts(yesterday(12,50)), 37, "pending", "closed",  "S.Fischer",  "N-QR 8901",  "Dachser",         30],
  [ts(yesterday(13,30)), 53, "closed",  "free",    "H.Schmidt",  null,         null,              120],
  [ts(yesterday(13,45)), 37, "closed",  "free",    "S.Fischer",  null,         null,              55],
  [ts(yesterday(14, 0)), 44, "free",    "closed",  "M.Wagner",   "BO-ST 3210", "Kühne+Nagel",     null],
  [ts(yesterday(14,30)), 57, "free",    "closed",  "C.Ziesch",   "MA-UV 7654", null,              null],
  [ts(yesterday(15,10)), 44, "closed",  "free",    "M.Wagner",   null,         null,              70],
  [ts(yesterday(15,40)), 57, "closed",  "free",    "C.Ziesch",   null,         null,              70],
  [ts(yesterday(16, 0)), 32, "free",    "closed",  "K.Müller",   "AC-WX 1098", null,              null],
  [ts(yesterday(16,45)), 32, "closed",  "free",    "K.Müller",   null,         null,              45],
  [ts(yesterday(17, 5)), 55, "free",    "closed",  "T.Bauer",    "KA-YZ 5432", "Rewe Logistik",   null],
  [ts(yesterday(17,50)), 55, "closed",  "free",    "T.Bauer",    null,         null,              45],

  // ── Today (2026-05-08) ────────────────────────────────────────────────────
  [ts(hoursAgo(6, 10)), 30, "free",    "closed",  "H.Schmidt",  "HH-CD 9012", "DB Schenker",     null],
  [ts(hoursAgo(6, 45)), 30, "closed",  "free",    "H.Schmidt",  null,         null,              35],
  [ts(hoursAgo(7,  0)), 36, "free",    "closed",  "K.Müller",   "M-XY 5678",  null,              null],
  [ts(hoursAgo(7, 15)), 40, "free",    "closed",  "M.Wagner",   "B-AB 1234",  "Rewe Logistik",   null],
  [ts(hoursAgo(7, 30)), 36, "closed",  "free",    "K.Müller",   null,         null,              30],
  [ts(hoursAgo(7, 45)), 33, "free",    "pending", "S.Fischer",  "K-EF 3456",  "DHL Express",     null],
  [ts(hoursAgo(8, 15)), 33, "pending", "closed",  "S.Fischer",  "K-EF 3456",  "DHL Express",     30],
  [ts(hoursAgo(8, 30)), 40, "closed",  "free",    "M.Wagner",   null,         null,              75],
  [ts(hoursAgo(8, 45)), 45, "free",    "closed",  "C.Ziesch",   "S-IJ 2345",  null,              null],
  [ts(hoursAgo(9,  0)), 48, "free",    "closed",  "T.Bauer",    "F-GH 7890",  "Rhenus",          null],
  [ts(hoursAgo(9, 20)), 33, "closed",  "free",    "S.Fischer",  null,         null,              65],
  [ts(hoursAgo(9, 40)), 45, "closed",  "free",    "C.Ziesch",   null,         null,              55],
  [ts(hoursAgo(9, 55)), 51, "free",    "closed",  "K.Müller",   "D-KL 6789",  "Dachser",         null],
  [ts(hoursAgo(10,10)), 48, "closed",  "free",    "T.Bauer",    null,         null,              70],
  [ts(hoursAgo(10,25)), 54, "free",    "closed",  "H.Schmidt",  "L-MN 0123",  null,              null],
  [ts(hoursAgo(10,50)), 51, "closed",  "free",    "K.Müller",   null,         null,              55],
  [ts(hoursAgo(11, 5)), 39, "free",    "pending", "S.Fischer",  "N-QR 8901",  "Kühne+Nagel",     null],
  [ts(hoursAgo(11,35)), 39, "pending", "closed",  "S.Fischer",  "N-QR 8901",  "Kühne+Nagel",     30],
  [ts(hoursAgo(11,50)), 54, "closed",  "free",    "H.Schmidt",  null,         null,              85],
  [ts(hoursAgo(12, 5)), 39, "closed",  "free",    "S.Fischer",  null,         null,              30],
  [ts(hoursAgo(12,20)), 56, "free",    "closed",  "M.Wagner",   "BO-ST 3210", null,              null],
  [ts(hoursAgo(12,45)), 34, "free",    "closed",  "C.Ziesch",   "MA-UV 7654", "DB Schenker",     null],
  [ts(hoursAgo(13,15)), 56, "closed",  "free",    "M.Wagner",   null,         null,              55],
  [ts(hoursAgo(13,30)), 34, "closed",  "free",    "C.Ziesch",   null,         null,              45],
  [ts(hoursAgo(13,50)), 47, "free",    "closed",  "K.Müller",   "AC-WX 1098", "Rewe Logistik",   null],
  [ts(hoursAgo(14,20)), 47, "closed",  "free",    "K.Müller",   null,         null,              30],
  [ts(hoursAgo(14,35)), 52, "free",    "closed",  "T.Bauer",    "KA-YZ 5432", null,              null],
  [ts(hoursAgo(15, 0)), 52, "closed",  "free",    "T.Bauer",    null,         null,              25],
  [ts(hoursAgo(15,15)), 41, "free",    "closed",  "H.Schmidt",  "DO-AA 9876", "Dachser",         null],
  [ts(hoursAgo(15,45)), 41, "closed",  "free",    "H.Schmidt",  null,         null,              30],
];

const insertMany = db.transaction((rows: EventRow[]) => {
  for (const row of rows) insertEvent.run(...row);
});
insertMany(events);
console.log(`✓ ${events.length} ramp_events inserted`);

// ── chat_messages ─────────────────────────────────────────────────────────────

const insertMsg = db.prepare(`
  INSERT OR IGNORE INTO chat_messages (id, user, text, timestamp)
  VALUES (?, ?, ?, ?)
`);

const chatData: [string, string, string, string][] = [
  // Yesterday
  ["czies-y001", "C.Ziesch",  "Guten Morgen! Rampe 43 ist frei, bitte koordinieren.",        ts(yesterday(7, 5))],
  ["kmuel-y002", "K.Müller",  "Alles klar, ich leite den Rewe-LKW zu Rampe 35.",             ts(yesterday(7, 8))],
  ["hschm-y003", "H.Schmidt", "LKW HH-CD 9012 steht an Rampe 42, ca. 30 min.",              ts(yesterday(6, 20))],
  ["sfisch-y004","S.Fischer", "Rampe 31 für DHL reserviert – kommt um 8:15 Uhr.",            ts(yesterday(8, 10))],
  ["mwagn-y005", "M.Wagner",  "Schenker-LKW an Rampe 38 fertig, Rampe wieder frei.",        ts(yesterday(9, 12))],
  ["tbauer-y006","T.Bauer",   "Achtung: Rampe 46 blockiert durch defekte Palette.",          ts(yesterday(9, 55))],
  ["czies-y007", "C.Ziesch",  "Ist bereinigt, danke für die Info!",                         ts(yesterday(10,35))],
  ["kmuel-y008", "K.Müller",  "Rhenus kommt mit 2 LKW, brauchen Rampe 50 + 51.",            ts(yesterday(10,48))],
  ["sfisch-y009","S.Fischer", "Rampe 51 ist leider noch belegt. Bitte 20 min warten.",      ts(yesterday(10,52))],
  ["hschm-y010", "H.Schmidt", "Mittagspause 12–13 Uhr: bitte keine neuen Lieferungen.",     ts(yesterday(11,55))],
  ["tbauer-y011","T.Bauer",   "Dachser-LKW meldet Verspätung, kommt erst 13:30 Uhr.",       ts(yesterday(12,25))],
  ["mwagn-y012", "M.Wagner",  "Verstanden, Rampe 37 bleibt bis dahin reserviert.",          ts(yesterday(12,30))],
  ["czies-y013", "C.Ziesch",  "Kühne+Nagel Abholung Rampe 44 abgeschlossen.",               ts(yesterday(14,12))],
  ["kmuel-y014", "K.Müller",  "Rampe 32 fertig entladen – alles okay.",                     ts(yesterday(16,50))],
  ["sfisch-y015","S.Fischer", "Rewe Logistik Rampe 55 abgewickelt, Fahrer zufrieden.",      ts(yesterday(17,52))],
  ["hschm-y016", "H.Schmidt", "Schichtende. Alle Rampen frei außer 53. Gute Nacht!",        ts(yesterday(17,58))],

  // Today
  ["hschm-t001", "H.Schmidt", "Guten Morgen! Erster LKW DB Schenker schon an Rampe 30.",   ts(hoursAgo(6, 12))],
  ["kmuel-t002", "K.Müller",  "Bin da, übernehme Rampe 36.",                               ts(hoursAgo(7,  2))],
  ["mwagn-t003", "M.Wagner",  "Rewe LKW an Rampe 40, Kennzeichen B-AB 1234.",              ts(hoursAgo(7, 18))],
  ["sfisch-t004","S.Fischer", "DHL bestätigt: kommen um 7:45 Uhr an Rampe 33.",            ts(hoursAgo(7, 40))],
  ["czies-t005", "C.Ziesch",  "Rampe 45 besetzt. Kurze Wartezeit.",                        ts(hoursAgo(8, 48))],
  ["tbauer-t006","T.Bauer",   "Rhenus Fahrer fragt nach Rampe 48 – schon geöffnet.",        ts(hoursAgo(9,  3))],
  ["sfisch-t007","S.Fischer", "Ja, Rampe 48 ist jetzt frei.",                              ts(hoursAgo(9, 42))],
  ["kmuel-t008", "K.Müller",  "Dachser LKW kommt jetzt an Rampe 51.",                      ts(hoursAgo(9, 53))],
  ["hschm-t009", "H.Schmidt", "Rampe 54 beladen, ca. 45 min.",                             ts(hoursAgo(10,28))],
  ["czies-t010", "C.Ziesch",  "Achtung: Rampe 39 für Kühne+Nagel reserviert ab 11 Uhr.",  ts(hoursAgo(10,55))],
  ["mwagn-t011", "M.Wagner",  "Verstanden, halte die Rampe frei.",                         ts(hoursAgo(11, 0))],
  ["sfisch-t012","S.Fischer", "Kühne+Nagel Abwicklung läuft, alles reibungslos.",          ts(hoursAgo(11,40))],
  ["tbauer-t013","T.Bauer",   "Mittagspause beginnt. Rampe 52 für 25 min belegt.",         ts(hoursAgo(12,22))],
  ["kmuel-t014", "K.Müller",  "Rewe LKW an Rampe 47 entladen. Sehr schnell heute!",       ts(hoursAgo(13,52))],
  ["hschm-t015", "H.Schmidt", "Dachser hat Rampe 41 angefordert – bestätigt.",             ts(hoursAgo(15,12))],
  ["czies-t016", "C.Ziesch",  "Stand 15:50 Uhr: 16 Abwicklungen heute. Guter Tag!",       ts(hoursAgo(15,50))],
];

const insertMsgs = db.transaction((rows: typeof chatData) => {
  for (const row of rows) insertMsg.run(...row);
});
insertMsgs(chatData);
console.log(`✓ ${chatData.length} chat_messages inserted`);

// ── bump state_version so frontend picks up changes ───────────────────────────
db.exec("UPDATE meta SET value = value + 1 WHERE key = 'state_version'");
console.log("✓ state_version bumped");

db.close();
console.log("\nDone. Restart the app or wait for the next poll cycle.");

const express = require('express');
const http = require('http');
const { Server } = require('socket.io');
const cors = require('cors');
const sqlite3 = require('sqlite3').verbose();
const path = require('path');

const app = express();
app.use(cors());

const server = http.createServer(app);
const io = new Server(server, {
  cors: {
    origin: "*", // allow all origins since it's an internal company tool run locally
    methods: ["GET", "POST"]
  }
});

// Setup SQLite Database
const dbPath = path.join(__dirname, 'database.sqlite');
const db = new sqlite3.Database(dbPath, (err) => {
  if (err) {
    console.error("Error opening database:", err.message);
  } else {
    console.log("Connected to the SQLite database.");
    initDB();
  }
});

function initDB() {
  db.run(`CREATE TABLE IF NOT EXISTS ramps (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    last_updated_by TEXT NOT NULL,
    last_updated_at DATETIME
  )`, (err) => {
    if (err) {
      console.error("Error creating table:", err.message);
      return;
    }
    
    // Check if empty, if so, populate 50 ramps
    db.get("SELECT COUNT(*) as count FROM ramps", (err, row) => {
      if (err) return console.error(err);
      if (row.count === 0) {
        console.log("Populating database with 50 initial ramps...");
        const stmt = db.prepare("INSERT INTO ramps (id, name, status, last_updated_by, last_updated_at) VALUES (?, ?, ?, ?, ?)");
        
        db.serialize(() => {
          for (let i = 1; i <= 50; i++) {
            stmt.run(i, `Ramp ${i}`, 'free', 'System', null);
          }
          stmt.finalize();
          console.log("Initial ramps inserted.");
        });
      }
    });
  });
}

// WebSocket connection handling
io.on('connection', (socket) => {
  console.log(`User connected: ${socket.id}`);

  // Send initial data to the newly connected user
  db.all("SELECT * FROM ramps ORDER BY id ASC", [], (err, rows) => {
    if (err) {
      console.error(err.message);
      return;
    }
    // format last_updated_at string to Date if needed, but strings are fine for JSON
    socket.emit('initial_data', rows);
  });

  // Handle ramp status updates
  socket.on('update_ramp', (data) => {
    const { id, status, last_updated_by, last_updated_at } = data;
    
    // Update DB
    db.run(
      `UPDATE ramps SET status = ?, last_updated_by = ?, last_updated_at = ? WHERE id = ?`,
      [status, last_updated_by, last_updated_at, id],
      function (err) {
        if (err) {
          console.error("Error updating ramp:", err.message);
          return;
        }
        
        // Broadcast the update to all clients (including sender) so their UI stays in perfect sync
        io.emit('ramp_updated', data);
      }
    );
  });

  socket.on('disconnect', () => {
    console.log(`User disconnected: ${socket.id}`);
  });
});

const PORT = 3000;
server.listen(PORT, '0.0.0.0', () => {
  console.log(`Ramp Sync Server running on http://0.0.0.0:${PORT}`);
});

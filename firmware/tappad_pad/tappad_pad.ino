// TapPad pad firmware: ESP32 + RC522 over SPI, two piezo buzzers.
//
// The pad does one thing: read a card UID and print one JSON line per event
// over USB serial at 115200 baud. It never decides whether a payment is
// approved; the server does. Shapes are in docs/protocol.md and pinned by
// crates/tappad-bridge/tests/firmware_lines.rs.
//
//   {"event":"ready","firmware":"0.1.0"}
//   {"event":"tap","uid":"04A3B2C1"}
//   {"event":"error","message":"reader not found"}
//
// Library: "MFRC522" by GithubCommunity (Arduino Library Manager).
// Board: ESP32 Dev Module.

#include <SPI.h>
#include <MFRC522.h>

// ── Pin config ────────────────────────────────────────────────
#define SS_PIN    5
#define RST_PIN   27
#define BUZZER_A  25   // high-pitch lead
#define BUZZER_B  26   // low resonance
// ─────────────────────────────────────────────────────────────

static const char FIRMWARE_VERSION[] = "0.1.0";

// After a tap, ignore the reader this long so one hold is one event.
static const unsigned long COOLDOWN_MS = 1500;

MFRC522 rfid(SS_PIN, RST_PIN);

// Short blip on both buzzers so the player knows the tap registered.
// The approval "ding" belongs to the game once the server says paid; the pad
// does not know the outcome and must not pretend to.
void blip(unsigned int hz, unsigned long ms) {
  tone(BUZZER_A, hz, ms);
  tone(BUZZER_B, hz, ms);
  delay(ms + 20);
  noTone(BUZZER_A);
  noTone(BUZZER_B);
}

void printReady() {
  Serial.print("{\"event\":\"ready\",\"firmware\":\"");
  Serial.print(FIRMWARE_VERSION);
  Serial.println("\"}");
}

void printError(const char* message) {
  Serial.print("{\"event\":\"error\",\"message\":\"");
  Serial.print(message);
  Serial.println("\"}");
}

// UID bytes as uppercase hex, no separators: 04 A3 B2 C1 -> "04A3B2C1".
void printTap(const MFRC522::Uid& uid) {
  static const char HEX_DIGITS[] = "0123456789ABCDEF";
  Serial.print("{\"event\":\"tap\",\"uid\":\"");
  for (byte i = 0; i < uid.size; i++) {
    Serial.print(HEX_DIGITS[uid.uidByte[i] >> 4]);
    Serial.print(HEX_DIGITS[uid.uidByte[i] & 0x0F]);
  }
  Serial.println("\"}");
}

void setup() {
  Serial.begin(115200);
  SPI.begin();
  rfid.PCD_Init();

  // VersionReg reads 0x00 or 0xFF when the RC522 is not wired or not powered.
  byte version = rfid.PCD_ReadRegister(MFRC522::VersionReg);
  if (version == 0x00 || version == 0xFF) {
    printError("reader not found");
  }
  printReady();
}

void loop() {
  if (!rfid.PICC_IsNewCardPresent() || !rfid.PICC_ReadCardSerial()) {
    return;
  }

  printTap(rfid.uid);
  blip(1800, 60);

  rfid.PICC_HaltA();
  rfid.PCD_StopCrypto1();
  delay(COOLDOWN_MS);
}

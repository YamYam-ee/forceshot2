# TRK ForceShot 2

Skärmklipp och bildredigering för provningsdokumentation, omskriven från grunden
i Rust med egui – samma teknik som TRK PDF Flow. En kodbas för **Windows och
macOS**.

Version 1 (C#/WPF) är ett eget program med egna utgåvor.

## Vad som är nytt

- **Startar på under en halv sekund och väger 16 MB** (version 1: 73 MB). Ritytan ritas på
  grafikkortet.
- **Ligger i aktivitetsfältet utan fönster** tills något behövs.
- **Nytt utseende och ny logga**: grafitgrått med turkos accent, egna ikoner.
- **Skisserna följer med**: Bottenskiss och Tankskiss är inbyggda i programfilen
  och öppnas som flikar. *Till bildredigeraren* i tankskissen lägger bilden som
  en ny flik, med skalan för filmmarkeringar redan satt.

## Installera

Kör **`TRK ForceShot Setup <version>.exe`** och klicka *Installera*. Inga
administratörsrättigheter behövs.

| | Plats |
|---|---|
| Programmet | `%LOCALAPPDATA%\TRK ForceShot 2\TRK ForceShot.exe` |
| Start-menyn | `TRK ForceShot 2` |
| Inställningar | `%APPDATA%\ForceShot2\settings.json` (rörs aldrig av en uppdatering) |
| Avinstallera | *Installerade appar* i Windows, eller `"TRK ForceShot.exe" --uninstall` |

Version 1 ligger i en egen mapp och rörs inte. Startar den också vid inloggning
erbjuder installationsfönstret att stänga av dess autostart, eftersom båda vill
ha PrintScreen.

**Ny version:** kör bara den nya Setup-filen. Den stänger den körande versionen,
byter ut filen och startar om. `dist\TRK ForceShot.exe` är samma program utan
installation.

**Bara en instans kör åt gången.** Startar du programmet igen, eller öppnar en
bild med det, tar den som redan kör hand om det.

### Uppdateringar

Vid start frågar programmet GitHub efter senaste utgåvan i repot som står under
**Inställningar → Allmänt** (förvalt `YamYam-ee/forceshot2`). Finns en nyare
version med en Setup-fil bifogad visas en ruta med *Ladda ner och uppdatera*,
*Senare* och *Hoppa över den här*. Hämtningen kontrolleras mot utgåvans SHA-256.

Så här lägger du upp en ny version:

1. `.\build.ps1 -Version 2.1.0`
2. Skapa en release i repot med taggen `v2.1.0`.
3. Bifoga `dist\TRK ForceShot Setup 2.1.0.exe`.
4. Beskrivningen är det användaren ser som "vad som är nytt".

Version 1 har sitt eget repo för utgåvor (`YamYam-ee/forceshot`), så de två
versionerna erbjuds aldrig varandras uppdateringar.

## Bygga

Kräver [Rust](https://rustup.rs).

```powershell
.\build.ps1          # Windows: dist\TRK ForceShot.exe och Setup-filen
```

```bash
./build-mac.sh       # macOS: dist/TRK ForceShot.app (körs på en Mac)
```

Ligger projektet i en synkad mapp (OneDrive, Dropbox) är det klokt att lägga
mellanfilerna utanför den: skapa `.cargo\config.toml` med `[build]` och
`target-dir = "..."`. Filen är undantagen från git.

## Startlägen

| Kommando | Gör |
|---|---|
| `TRK-ForceShot` | Öppnar fönstret |
| `TRK-ForceShot --tray` | Startar tyst i aktivitetsfältet (för autostart) |
| `TRK-ForceShot bild.png` | Öppnar en bild |
| `TRK-ForceShot --skiss tankskiss` | Öppnar en skiss direkt |
| `TRK-ForceShot --klipp` | Går rakt till markeringsläget |
| `TRK-ForceShot --ocr bild.png` | Öppnar bilden och läser av texten direkt |
| `TRK-ForceShot --demo` | Provbild med varje objekttyp (`--demo-ut fil.png` exporterar den) |

## Snabbtangenter

| Gör | Windows | Mac |
|---|---|---|
| Klipp ett område | `PrintScreen` eller `Ctrl+Alt+Shift+P` | `Ctrl+Alt+Shift+P` |
| Hela skärmen | `Ctrl+PrintScreen` eller `Ctrl+Alt+Shift+H` | `Ctrl+Alt+Shift+H` |
| Aktivt fönster | `Alt+PrintScreen` eller `Ctrl+Alt+Shift+F` | `Ctrl+Alt+Shift+F` |
| Samma område igen | `Shift+PrintScreen` eller `Ctrl+Alt+Shift+R` | `Ctrl+Alt+Shift+R` |

Ändras under **Inställningar → Snabbtangenter**. I redigeraren gäller samma
verktygstangenter som i version 1 (`V T R E L A H F W O M C`), plus `G` för
filmmarkering.

## Vad som finns och inte finns jämfört med version 1

Finns: skärmklipp (område, hel skärm, aktivt fönster, samma område),
förstoringsglas, flikar, text, rektangel, ellips, linje, pil, frihand,
överstrykning (multiplikation), oskärpa, mosaik, svetsskarvar med metoder och
löpnummer, filmmarkeringar med gemensam skala, beskär, rotera, spegelvänd,
vridning av objekt, ångra/gör om, kopiera/klistra in objekt och bilder,
spara som PNG/JPEG/BMP, båda skisserna, radera element, textavläsning (OCR) och
öppna PDF.

- **Radera element** (`D`): dra en ruta över det som ska bort. Ytan fylls i från
  omgivningen och går att flytta och ändra i efterhand. Bara ruta, ingen pensel.
- **Textavläsning**: knappen längst till höger i alternativraden. Läser svenska
  med Windows inbyggda textigenkänning och låter dig kopiera texten. Att byta ut
  en rad i bilden finns inte än, och på Mac saknas avläsningen helt.
- **PDF**: varje sida blir en egen flik (150 dpi, högst 40 sidor åt gången).
  Sidorna sparas som bilder; att spara tillbaka som PDF finns inte än.

**Inte flyttat än:** förstora/förbättra, fiberskiss, karta, ForceSpace, dela via mejl, högerklicksmenyn i
Utforskaren, dra ut flikar till
egna fönster, klick för att ta fönstret under pekaren, markering över flera
skärmar samtidigt.

## Läget på macOS

Mac-koden är typkontrollerad (`cargo check --target aarch64-apple-darwin`) men
**har aldrig körts på en Mac**. Räkna med justeringar första gången, främst i
`src/platform/macos.rs` (överläggets fönsternivå, skärmarnas koordinater) och i
`build-mac.sh`. macOS kräver behörigheten *Skärminspelning*, och utan ett Apple
Developer-certifikat måste den ges om efter varje nytt bygge.

## Koden

| Fil | Innehåll |
|---|---|
| `src/app.rs` | Händelseloop, fönster, aktivitetsfältsikon, flödet från genväg till bild |
| `src/capture.rs` | Markeringsöverlägget |
| `src/platform/` | Skärmdump och globala genvägar för Windows respektive Mac |
| `src/editor/mod.rs` | Fönstrets gränssnitt: flikar, rader, inställningar, spara/kopiera |
| `src/editor/canvas.rs` | Ritytan: verktyg, markering, handtag, textinmatning |
| `src/editor/doc.rs` | Dokument, objekt, ångra/gör om, geometri |
| `src/editor/draw.rs` | Ritning av objekten – samma kod för skärm och export |
| `src/editor/render.rs` | Export i full upplösning via grafikkortet |
| `src/install.rs` | Installation, avinstallation, autostart och en-instans-låset (Windows) |
| `src/update.rs` | Uppdateringskoll och hämtning från GitHub |
| `src/pdf.rs` | Öppnar PDF-filer sida för sida |
| `src/imaging.rs` | Oskärpa, mosaik, överstrykning och radering (inpainting) |
| `src/sketch.rs` | Skissflikarna (webbvy) och bryggan till redigeraren |
| `assets/sketches/` | Bottenskiss och Tankskiss |

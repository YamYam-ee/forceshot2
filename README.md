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

1. `.\build.ps1 -Version 2.2.1`
2. Skapa en release i repot med taggen `v2.2.1`.
3. Bifoga `dist\TRK ForceShot Setup 2.2.1.exe`.
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
| `TRK-ForceShot --fiberdemo [mapp]` | Två fiberblad av påhittade foton, exporteras till mappen |
| `TRK-ForceShot --ytademo [fil.png]` | En arbetsyta med innehåll i och utanför guiden |
| `TRK-ForceShot --pdfdemo fil.pdf` | Två fiberblad sparade som en tvåsidig PDF |
| `TRK-ForceShot --flikdemo` | Två fiberblad, det andra utdraget till ett eget fönster |
| `TRK-ForceShot --fonstertest` | Skriver ut skärmarna och fönstren som ett klick kan ta |
| `TRK-ForceShot --klipptest x,y,b,h fil.png` | Klipper ett område på skrivbordet, även över skärmgränser |
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

## Funktioner

**Skärmklipp:** område, hel skärm, aktivt fönster och samma område igen, med
förstoringsglas och åtgärdsrad. En markering får gå över flera skärmar, och ett
klick tar fönstret under pekaren (det lyfts fram när pekaren är över det).
*Ctrl+A* markerar skärmen, en gång till alla skärmar.

**Redigeraren:** flikar som kan byta plats och dras ut till egna fönster, text (vänster, centrerad, höger), rektangel, ellips,
linje, pil, frihand, överstrykning, oskärpa, mosaik, radera element, beskär,
rotera, spegelvänd, vridning av objekt, ångra/gör om, kopiera och klistra in
objekt och bilder.

**Provning:** svetsskarvar med metoder och löpnummer (brickan och pricken vid
skarven har var sin storlek), filmmarkeringar med
gemensam skala, Bottenskiss och Tankskiss.

- **Skarvnummer** frågas när skarven sätts ut. Kryssa i *Fortsätt med löpnummer
  härifrån* (eller *Löpnummer* i verktygsraden) så numreras resten av bilden av
  sig själv.
- **Fiberskiss** (plusknappen → *Fiberskiss*): fyra stående foton per blad med
  filnamnet som text i varje fack. Liggande bilder vänds. Bilderna går att
  flytta, förstora och vrida i sitt fack; den vita ramen ligger alltid överst.
  *Mer → Exportera bladen till skrivbordet* skriver alla blad som
  `<provningsnummer>-skiss-1`, `-2` … i valt format, kvalitet och bredd.
- **Arbetsyta / ForceSpace** (plusknappen → *Ny arbetsyta*): en vit yta där allt
  får ligga var som helst. Den streckade guiden (565 × 744) är det som blir bild.
  Guiden flyttas genom att dra i dess etikett; storleken är fast.
- **Förstora** (*Mer → Förstora*): 2×, 3× eller 4× med bildtyperna skärmklipp,
  inskannat dokument och foto. Annoteringarna följer med.
- **Komprimera** (*Mer → Komprimera och spara*): mindre JPEG med vald största
  sida och kvalitet.
- **Karta** (*Mer → Karta*): sök en adress, dra i kartan för att flytta utsnittet
  och rulla för att zooma. Det som syns läggs in som ett objekt med upphovstext.
  Kartan kommer från OpenStreetMap.
- **Dela via mejl** (*Mer → Dela via mejl*): öppnar ett nytt mejl med bilden
  bifogad. Ingenting skickas av appen.
- **Textavläsning (OCR):** läser svenska med Windows inbyggda textigenkänning.
- **PDF:** öppnas med en flik per sida (150 dpi, högst 40 sidor). En bild kan
  sparas som ensidig PDF, och *Mer → Spara flikar som PDF* gör en sida av varje
  ikryssad flik, med annoteringarna inbrända.
- **Flikar i egna fönster:** dra en flik ut ur flikraden och släpp den, eller
  högerklicka på fliken → *Öppna i eget fönster*. Släpps den på ett annat
  ForceShot-fönster hamnar den där. Inom raden byter flikarna plats.
- **Filformat:** öppnar PNG, JPEG, BMP, TIFF, GIF, WebP och PDF; sparar PNG,
  JPEG, BMP, TIFF, GIF och PDF.
- **Högerklick i Utforskaren:** *Redigera med TRK ForceShot* på bilder och
  PDF:er (i Windows 11 under *Visa fler alternativ*). Läggs dit av installationen.

### Skillnader mot version 1

- Förstora och komprimera saknar förhandsvisning före/efter.
- Arbetsytans guide kan flyttas men inte ändra storlek.
- Radera element finns bara som ruta, inte som pensel.
- OCR kan inte byta ut en rad i bilden.
- Snabbtangenter ändras genom att skriva kombinationen som text.
- En PDF som sparas består av bilder; texten i den går inte att markera.

## Läget på macOS

Utgåvorna har en Mac-fil, `TRK-ForceShot-mac-<version>.zip`, som byggs av
GitHub Actions (*Actions → Mac-bygge → Run workflow*, ange utgåvans tagg). Den går
på både Apple Silicon och Intel. Appen är inte signerad med ett Apple-certifikat, så
första gången måste den godkännas under *Systeminställningar → Integritet och
säkerhet → Öppna ändå*.

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
| `src/editor/tools.rs` | Fiberskiss, bladexport, förstora, komprimera, arbetsyta, karta, mejl |
| `src/map.rs` | Adressökning och kartbild från OpenStreetMap |
| `src/pdf.rs` | Öppnar PDF-filer sida för sida |
| `src/imaging.rs` | Oskärpa, mosaik, överstrykning och radering (inpainting) |
| `src/sketch.rs` | Skissflikarna (webbvy) och bryggan till redigeraren |
| `assets/sketches/` | Bottenskiss och Tankskiss |

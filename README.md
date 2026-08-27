# AutoSubs 3.0 Ultra 🚀

[Français en bas]

AutoSubs is a blazing-fast, API-first Rust backend and React frontend for auto-generating, styling, and burning subtitles onto videos. It natively supports multi-format (9:16, 16:9, 1:1, 4:5), local LLM spell-checking, and word-by-word animation techniques (Pop, Karaoke, Bounce).

### Features
* **Lightning Fast Backend**: Written in Rust (Axum + Tokio) + FFmpeg.
* **Smart Word-level ASS Generation**: Implements precise ASS tags (`{\k}`, `{\t}`) for viral karaoke and word-by-word 'pop' styles.
* **French Grammatical NLP**: Sophisticated regex and word-binding rules prevent awkward sentence breaks (e.g., keeps "parce que" together).
* **AI Auto-Correction**: Optionally run transcriptions through a local or remote LLM for perfect punctuation and spelling.
* **Advanced Workflows**: Multi-directory Watchdog. Drop a video in Folder A, it auto-burns and outputs to Folder B based on specific rules.
* **SaaS UI**: Responsive React 19 + Tailwind v4 interface with server-side file browser.

### API Usage
AutoSubs is completely API-driven. Trigger renders programmatically:
```bash
curl -X POST http://localhost:3000/api/burn \
  -H "Content-Type: application/json" \
  -d '{"videoId": "123", "presetName": "TikTok Viral"}'
```

### Docker Installation
```yaml
services:
  autosubs:
    image: ghcr.io/godsquantum/autosubs:latest
    container_name: autosubs-rs
    ports:
      - "3051:3000"
    volumes:
      - ./data:/app/data
      - ./fonts:/app/fonts
      - /mnt/NAS_SAL:/mnt/NAS_SAL
    restart: unless-stopped
```

### Workflow Configuration
Access `http://localhost:3051` -> **Workflows** tab. Use the visual file browser to map your NAS paths to specific styles. The Rust watchdog will handle the rest in the background.

---

# AutoSubs 3.0 Ultra (Français) 🇫🇷

AutoSubs est un moteur de sous-titrage ultra-rapide (Backend Rust / Frontend React) conçu pour générer, styliser et incruster des sous-titres. Il gère nativement le multiformat (9:16, 16:9, 1:1), la correction via LLM, et les animations virales mot-à-mot (Pop, Karaoké).

### Fonctionnalités
* **Backend Rust (Axum) + FFmpeg** : Des performances maximales sans bottleneck Node.js.
* **Styles ASS Viraux** : Génération native de balises ASS avancées (`{\k}`, `{\t}`) pour les effets "Pop" mot-par-mot et Karaoké.
* **NLP & Découpage Français** : Un moteur interne empêche les coupures de lignes absurdes (garde "parce que" ou "bien que" sur la même ligne).
* **Auto-Correction IA** : Relecture orthographique et ponctuation via LLM (local ou API).
* **Workflows (Watchdogs)** : Déposez une vidéo dans un dossier A, elle est traitée et envoyée dans un dossier B automatiquement.
* **UI SaaS** : Interface React 19 + Tailwind v4 fluide, avec explorateur de fichiers serveur intégré.

### Utilisation de l'API
Le front n'est qu'un client. Tout est scriptable :
```bash
curl -X POST http://localhost:3000/api/burn \
  -H "Content-Type: application/json" \
  -d '{"videoId": "123", "presetName": "TikTok Viral"}'
```

### Installation Docker
```yaml
services:
  autosubs:
    image: ghcr.io/godsquantum/autosubs:latest
    container_name: autosubs-rs
    ports:
      - "3051:3000"
    volumes:
      - ./data:/app/data
      - ./fonts:/app/fonts
      - /mnt/NAS_SAL:/mnt/NAS_SAL
    restart: unless-stopped
```

### Configuration des Workflows
Rendez-vous sur `http://localhost:3051` -> Onglet **Workflows**. Utilisez l'explorateur de fichiers visuel pour mapper les dossiers de votre NAS avec vos presets. Le watchdog Rust tourne en tâche de fond.

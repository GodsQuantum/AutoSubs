# AutoSubs Cut — revue comparative 15 octobre 2026

## Éléments vérifiés (sources éditeurs)

- [LosslessCut](https://github.com/mifi/lossless-cut) : timeline, points d'entrée/sortie (I/O), segments sauvegardés, visualisation des zones, affichage des passages retenus, contrôles temporels au clavier. Le mode copie sans réencodage est rapide mais peut déplacer une coupe au keyframe précédent ; ce compromis n'est pas acceptable comme défaut pour la précision du stand-up.
- [LosslessCut — limitations des coupes sans perte](https://github.com/mifi/lossless-cut/blob/master/docs/troubleshooting.md) : le mode smart cut entre images clés reste expérimental suivant codecs.
- [Shotcut 26.9](https://www.shotcutapp.com/download/releasenotes/) : édition sur timeline, trim et ripple. Plus adapté au montage multipiste qu'à la seule extraction de quelques passages.
- [Descript](https://feedback.descript.com/changelog/the-new-timeline-is-on) : timeline intégrée simplifiée ; le transcript peut également piloter le montage, mais nécessite un système supplémentaire de mapping des tokens aux médias.

## Décisions pour AutoSubs 3.6.1

1. Réutiliser le lecteur source, les classes de composants et l'API de Cut existants.
2. Ajouter **une** timeline de sélection tactile/souris sous la preview plutôt qu'un éditeur multipiste.
3. Marquer I/O depuis l'image affichée, offrir réglage numérique, clips ordonnés, aperçu continu des parties gardées, suppression/annulation, raccourcis non déclenchés dans les champs éditables.
4. Conserver la règle de sécurité : décodage/réencodage à la coupe exacte et nouveau MP4, **jamais de modification de la vidéo source**. Ne pas prétendre au lossless alors que l'export est H.264.
5. Persister le brouillon de sélection côté navigateur (par id du job) et le suivi des exports côté SQLite serveur ; une exportation interrompue doit être clairement signalée, pas présentée comme terminée.

## Scénarios de recette

- Marquer une fin avant le début et tenter d'ajouter une plage : refus.
- Sélectionner des intervalles en désordre : tri automatique ; chevauchements rejetés.
- Prévisualiser : jouer seulement les plages retenues puis arrêter proprement.
- Naviguer / éditer un texte de sous-titres : les raccourcis I/O ne doivent pas interférer.
- Recharger l'interface ou redémarrer le service après une coupe terminée : liens et état récupérés.
- Exporter 1–2 s et 4–6 s d'une même source : MP4 ~3 s, audio synchronisé et source intouchée.
- Codes CI Rust/frontend et tests de sécurité existants restent verts.

## Écart intentionnel

Pas de multipiste, de waveform générée au serveur, de reverse/timescale ni de keyframe-only default. Descript texte-à-vidéo mérite un projet spécifique si demandé ; la sélection temporelle couvre la demande actuelle.

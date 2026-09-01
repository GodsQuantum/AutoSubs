import { derived, writable } from 'svelte/store';
import type { Locale } from './types';

const en = {
  queue:'Queue', editor:'Editor', presets:'Presets', brands:'Brands', workflows:'Workflows', settings:'Settings',
  tagline:'Subtitle production, without the busywork.', english:'EN', french:'FR', language:'Language', uiLanguage:'Interface language',
  addFiles:'Add files', addServerVideo:'Add server video', dropFiles:'Drop videos here', dropHint:'Video only or video + .srt / .ass / .ssa / .json', upload:'Upload', uploading:'Uploading',
  uploadQueue:'Upload queue', uploadProgress:'Upload progress', noJobs:'No jobs yet', noJobsHint:'Drop a video or choose files to start.', edit:'Edit', render:'Render', renderVideo:'Render video', cancel:'Cancel', retry:'Retry', prepare:'Prepare',
  source:'Source', status:'Status', progress:'Progress', output:'Output', error:'Error', updated:'Updated', sidecar:'Sidecar', none:'None', created:'Created',
  save:'Save', saved:'Saved', delete:'Delete', split:'Split', mergePrevious:'Merge previous', mergeNext:'Merge next', retranscribe:'Retranscribe', reRender:'Re-render', create:'Create', close:'Close', select:'Select', browse:'Browse', back:'Back', refresh:'Refresh', new:'New', duplicate:'Duplicate',
  videoPreview:'Video preview', subtitles:'Subtitles', search:'Search', replace:'Replace', replaceAll:'Replace all', shift:'Shift', milliseconds:'ms', seconds:'seconds',
  regroup:'Regroup', maxChars:'Max chars / line', maxLines:'Max lines', export:'Export', exportSrt:'SRT', exportAss:'ASS', exportJson:'JSON', timingRepair:'Timing repair', timingRepairHint:'The backend normalizes overlaps and word timing when you save.',
  repairedOverlaps:'Repaired overlaps', retimedLines:'Retimed word lines', droppedEmpty:'Dropped empty lines', currentTime:'Current time', jumpToLine:'Jump to line',
  attachedSidecar:'Attached sidecar', attachFile:'Upload sidecar', chooseServerFile:'Choose server file', removeSidecar:'Remove sidecar', replaceSidecar:'Replace sidecar',
  preset:'Preset', format:'Format', fit:'Fit', sourceFormat:'Source', portrait916:'9:16', landscape169:'16:9', square11:'1:1', portrait45:'4:5', custom:'Custom',
  preserve:'Preserve', contain:'Contain', cover:'Cover', stretch:'Stretch', width:'Width', height:'Height', apply:'Apply', applyToJob:'Apply to job',
  newPreset:'New preset', presetName:'Preset name', animation:'Animation', font:'Font', size:'Size', positionX:'Position X', positionY:'Position Y',
  baseColor:'Base color', highlightColor:'Highlight', outlineColor:'Outline', outline:'Outline size', shadow:'Shadow', shadowColor:'Shadow color', uppercase:'Uppercase', bold:'Bold', italic:'Italic', floating:'Floating', keywords:'Filename keywords', lineSpacing:'Line spacing', wobbleSpeed:'Floating speed', borderStyle:'Border style',
  preview:'Preview', sampleText:'THIS IS A PREVIEW', brand:'Brand', noBrand:'No brand', outro:'Outro', defaultOutro:'Default outro', noOutro:'No outro',
  newBrand:'New brand', brandName:'Brand name', description:'Description', brandDefaults:'Default preset per format', assets:'Assets', importAsset:'Import server asset', uploadAsset:'Upload asset', logo:'Logo', assetLibrary:'Asset library',
  newWorkflow:'New workflow', workflowName:'Workflow name', watchDir:'Watch folder', outputDir:'Output folder', archiveDir:'Archive folder', enabled:'Enabled', disabled:'Disabled', presetOverride:'Preset override', brandDefault:'Brand default', workflowHint:'Native filesystem events plus periodic reconciliation keep NFS workflows reliable.',
  transcription:'Transcription', primary:'Primary', localFallback:'Local / fallback', endpoint:'Endpoint', model:'Model', apiKey:'API key', keyStored:'Key stored', keepKey:'Keep stored key', replaceKey:'Replace key', clearKey:'Clear key',
  transcriptionLanguage:'Transcription language', localEnabled:'Use local transcription first', fallbackEnabled:'Fallback to the other provider on failure', llm:'LLM correction', llmEnabled:'Enable correction', prompt:'Correction prompt',
  encoding:'Encoding', encoder:'Encoder', quality:'Quality', encoderPreset:'Encoder preset', capabilities:'Capabilities', ffmpeg:'FFmpeg', libass:'libass', available:'Available', unavailable:'Unavailable', detected:'Detected',
  filePicker:'Server picker', folder:'Folder', file:'File', choose:'Choose', root:'Root', noEntries:'No matching entries', currentPath:'Current path', filter:'Filter',
  pending:'Pending', uploadingStatus:'Uploading', probing:'Probing', transcribing:'Transcribing', correcting:'Correcting', ready:'Ready', rendering:'Rendering', done:'Done', cancelled:'Cancelled', interrupted:'Interrupted', failed:'Failed',
  loading:'Loading…', saving:'Saving…', confirmDelete:'Delete this item?', required:'Required', copied:'Copied', open:'Open', clear:'Clear',
  settingsInfo:'Secrets are never returned by the API. Leaving a key untouched keeps the stored value.',
  mobileTip:'Everything remains editable on phone and tablet; panels collapse instead of removing controls.',
  pathNotAllowed:'Path is outside allowed roots.', uploadFailed:'Upload failed', operationFailed:'Operation failed', connectionLost:'Connection lost. AutoSubs will retry when the page refreshes.',
  queueReadyHint:'Ready means subtitles are prepared; no video re-encode has happened yet.',
  sourcePreserveHint:'Source + Preserve keeps the original dimensions and aspect ratio.',
  serverFileHint:'Use the server picker when the video is already mounted into AutoSubs. Nothing is copied before processing.',
  noSelectedJob:'Select a job from Queue to edit it.', activeJobLocked:'This job is active. Editing unlocks when preparation/rendering stops.',
  typography:'Typography', placement:'Placement', segmentation:'Segmentation', styling:'Styling', outputSection:'Output',
  videoOnly:'Video only', paired:'Paired with subtitle', jobs:'jobs', fileCount:'files',
  auto:'Auto', libx264:'libx264', libx265:'libx265', nvencH264:'NVENC H.264', nvencHevc:'NVENC HEVC', qsvH264:'Intel QSV H.264', vaapiH264:'VA-API H.264', amfH264:'AMD AMF H.264',
  pop:'Pop', highlight:'Highlight', karaoke:'Karaoke', fade:'Fade', slideUp:'Slide up', bounce:'Bounce', animationNone:'None',
  overview:'Overview', provider:'Provider', performance:'Performance', advanced:'Advanced',
  localProvider:'Local provider', primaryProvider:'Primary provider', testModels:'List models',
  remove:'Remove', chooseAsset:'Choose asset', assetName:'Asset name', noAssets:'No assets yet', noPresets:'No presets yet', noBrands:'No brands yet', noWorkflows:'No workflows yet',
  saveBeforeRender:'Save subtitle edits before rendering.', renderComplete:'Render complete', prepared:'Prepared',
  appName:'AutoSubs'
};

const fr: typeof en = {
  queue:'File', editor:'Éditeur', presets:'Presets', brands:'Marques', workflows:'Workflows', settings:'Réglages',
  tagline:'La production de sous-titres, sans les tâches répétitives.', english:'EN', french:'FR', language:'Langue', uiLanguage:"Langue de l’interface",
  addFiles:'Ajouter des fichiers', addServerVideo:'Ajouter une vidéo serveur', dropFiles:'Déposez vos vidéos ici', dropHint:'Vidéo seule ou vidéo + .srt / .ass / .ssa / .json', upload:'Importer', uploading:'Import en cours',
  uploadQueue:"File d’import", uploadProgress:"Progression de l’import", noJobs:'Aucun job', noJobsHint:'Déposez une vidéo ou choisissez des fichiers pour commencer.', edit:'Éditer', render:'Rendre', renderVideo:'Rendre la vidéo', cancel:'Annuler', retry:'Réessayer', prepare:'Préparer',
  source:'Source', status:'Statut', progress:'Progression', output:'Sortie', error:'Erreur', updated:'Mis à jour', sidecar:'Sous-titre associé', none:'Aucun', created:'Créé',
  save:'Enregistrer', saved:'Enregistré', delete:'Supprimer', split:'Scinder', mergePrevious:'Fusionner précédent', mergeNext:'Fusionner suivant', retranscribe:'Retranscrire', reRender:'Rerendre', create:'Créer', close:'Fermer', select:'Sélectionner', browse:'Parcourir', back:'Retour', refresh:'Actualiser', new:'Nouveau', duplicate:'Dupliquer',
  videoPreview:'Aperçu vidéo', subtitles:'Sous-titres', search:'Rechercher', replace:'Remplacer', replaceAll:'Tout remplacer', shift:'Décaler', milliseconds:'ms', seconds:'secondes',
  regroup:'Regrouper', maxChars:'Caractères max / ligne', maxLines:'Lignes max', export:'Exporter', exportSrt:'SRT', exportAss:'ASS', exportJson:'JSON', timingRepair:'Réparation des timings', timingRepairHint:'Le backend normalise les chevauchements et les timings des mots à l’enregistrement.',
  repairedOverlaps:'Chevauchements réparés', retimedLines:'Lignes de mots recalées', droppedEmpty:'Lignes vides retirées', currentTime:'Temps courant', jumpToLine:'Aller à la ligne',
  attachedSidecar:'Sous-titre associé', attachFile:'Importer un sous-titre', chooseServerFile:'Choisir sur le serveur', removeSidecar:'Retirer le sous-titre', replaceSidecar:'Remplacer le sous-titre',
  preset:'Preset', format:'Format', fit:'Ajustement', sourceFormat:'Source', portrait916:'9:16', landscape169:'16:9', square11:'1:1', portrait45:'4:5', custom:'Personnalisé',
  preserve:'Conserver', contain:'Contenir', cover:'Couvrir', stretch:'Étirer', width:'Largeur', height:'Hauteur', apply:'Appliquer', applyToJob:'Appliquer au job',
  newPreset:'Nouveau preset', presetName:'Nom du preset', animation:'Animation', font:'Police', size:'Taille', positionX:'Position X', positionY:'Position Y',
  baseColor:'Couleur de base', highlightColor:'Surbrillance', outlineColor:'Contour', outline:'Taille du contour', shadow:'Ombre', shadowColor:"Couleur de l’ombre", uppercase:'Majuscules', bold:'Gras', italic:'Italique', floating:'Flottant', keywords:'Mots-clés du nom de fichier', lineSpacing:'Espacement des lignes', wobbleSpeed:'Vitesse flottante', borderStyle:'Style de bordure',
  preview:'Aperçu', sampleText:'CECI EST UN APERÇU', brand:'Marque', noBrand:'Aucune marque', outro:'Outro', defaultOutro:'Outro par défaut', noOutro:'Aucun outro',
  newBrand:'Nouvelle marque', brandName:'Nom de la marque', description:'Description', brandDefaults:'Preset par défaut selon le format', assets:'Assets', importAsset:'Importer un asset serveur', uploadAsset:'Importer un asset', logo:'Logo', assetLibrary:"Bibliothèque d’assets",
  newWorkflow:'Nouveau workflow', workflowName:'Nom du workflow', watchDir:'Dossier surveillé', outputDir:'Dossier de sortie', archiveDir:"Dossier d’archive", enabled:'Activé', disabled:'Désactivé', presetOverride:'Forcer un preset', brandDefault:'Défaut de la marque', workflowHint:'Les événements natifs et une réconciliation périodique rendent les workflows fiables aussi sur NFS.',
  transcription:'Transcription', primary:'Principal', localFallback:'Local / secours', endpoint:'Endpoint', model:'Modèle', apiKey:'Clé API', keyStored:'Clé enregistrée', keepKey:'Conserver la clé', replaceKey:'Remplacer la clé', clearKey:'Effacer la clé',
  transcriptionLanguage:'Langue de transcription', localEnabled:'Utiliser d’abord la transcription locale', fallbackEnabled:'Basculer vers l’autre fournisseur en cas d’échec', llm:'Correction LLM', llmEnabled:'Activer la correction', prompt:'Prompt de correction',
  encoding:'Encodage', encoder:'Encodeur', quality:'Qualité', encoderPreset:'Preset encodeur', capabilities:'Capacités', ffmpeg:'FFmpeg', libass:'libass', available:'Disponible', unavailable:'Indisponible', detected:'Détecté',
  filePicker:'Explorateur serveur', folder:'Dossier', file:'Fichier', choose:'Choisir', root:'Racine', noEntries:'Aucun élément correspondant', currentPath:'Chemin courant', filter:'Filtrer',
  pending:'En attente', uploadingStatus:'Import', probing:'Analyse', transcribing:'Transcription', correcting:'Correction', ready:'Prêt', rendering:'Rendu', done:'Terminé', cancelled:'Annulé', interrupted:'Interrompu', failed:'Échec',
  loading:'Chargement…', saving:'Enregistrement…', confirmDelete:'Supprimer cet élément ?', required:'Obligatoire', copied:'Copié', open:'Ouvrir', clear:'Effacer',
  settingsInfo:'Les secrets ne sont jamais renvoyés par l’API. Ne pas toucher à une clé conserve sa valeur enregistrée.',
  mobileTip:'Tout reste modifiable sur téléphone et tablette : les panneaux se replient sans supprimer de contrôles.',
  pathNotAllowed:'Le chemin est hors des racines autorisées.', uploadFailed:"Échec de l’import", operationFailed:"Échec de l’opération", connectionLost:'Connexion perdue. AutoSubs reprendra lors du prochain rafraîchissement.',
  queueReadyHint:'Prêt signifie que les sous-titres sont préparés ; aucun réencodage vidéo n’a encore eu lieu.',
  sourcePreserveHint:'Source + Conserver garde les dimensions et le ratio originaux.',
  serverFileHint:'Utilisez le picker serveur si la vidéo est déjà montée dans AutoSubs. Rien n’est recopié avant le traitement.',
  noSelectedJob:'Sélectionnez un job dans la File pour le modifier.', activeJobLocked:'Ce job est actif. L’édition se déverrouille à la fin de la préparation ou du rendu.',
  typography:'Typographie', placement:'Placement', segmentation:'Segmentation', styling:'Style', outputSection:'Sortie',
  videoOnly:'Vidéo seule', paired:'Avec sous-titre', jobs:'jobs', fileCount:'fichiers',
  auto:'Auto', libx264:'libx264', libx265:'libx265', nvencH264:'NVENC H.264', nvencHevc:'NVENC HEVC', qsvH264:'Intel QSV H.264', vaapiH264:'VA-API H.264', amfH264:'AMD AMF H.264',
  pop:'Pop', highlight:'Surbrillance', karaoke:'Karaoké', fade:'Fondu', slideUp:'Glissé haut', bounce:'Rebond', animationNone:'Aucune',
  overview:'Vue d’ensemble', provider:'Fournisseur', performance:'Performances', advanced:'Avancé',
  localProvider:'Fournisseur local', primaryProvider:'Fournisseur principal', testModels:'Lister les modèles',
  remove:'Retirer', chooseAsset:'Choisir un asset', assetName:"Nom de l’asset", noAssets:'Aucun asset', noPresets:'Aucun preset', noBrands:'Aucune marque', noWorkflows:'Aucun workflow',
  saveBeforeRender:'Enregistrez les modifications des sous-titres avant le rendu.', renderComplete:'Rendu terminé', prepared:'Préparé',
  appName:'AutoSubs'
};

export type Dictionary = typeof en;
export const locale = writable<Locale>('en');
export const dictionary = derived(locale, ($locale) => $locale === 'fr' ? fr : en);

export function setLocale(value: Locale) {
  locale.set(value);
  if (typeof localStorage !== 'undefined') localStorage.setItem('autosubs:uiLanguage', value);
  if (typeof document !== 'undefined') document.documentElement.lang = value;
}

export function initLocale() {
  if (typeof window === 'undefined') return;
  const saved = localStorage.getItem('autosubs:uiLanguage') as Locale | null;
  const detected: Locale = navigator.language.toLowerCase().startsWith('fr') ? 'fr' : 'en';
  setLocale(saved === 'fr' || saved === 'en' ? saved : detected);
}

# git-exile

Cliente gráfico de git escrito en Rust con [egui](https://github.com/emilk/egui).

Permite ver el historial de un repositorio, revisar los cambios de cada commit y crear commits nuevos.
Para trabajar con repositorios no usa la API de GitHub, GitLab ni de ningún otro servicio: ejecuta el binario
`git` instalado en el sistema, por lo que usa tu configuración de git tal cual (`user.name`, `user.email`, hooks,
firma GPG/SSH, credential helpers, etc.). Solo se conecta a la API de GitHub para buscar actualizaciones de la
propia aplicación.

## Funcionalidades

- Historial de todas las ramas, remotos y tags con grafo de ramas y merges.
- Columna con la rama de cada commit: la punta de cada rama siempre visible y, al pasar el ratón, la rama
  de los commits anteriores (más transparente). Se deduce siguiendo el primer padre y los mensajes de merge.
- Etiquetas de ramas, ramas remotas, tags y HEAD en cada commit.
- Indicador de dónde está alojado el repositorio (GitHub, GitLab, otro servidor o solo local), deducido de
  sus remotos.
- Carga paginada del historial, pensada para repositorios grandes.
- Detalles del commit: autor, committer, mensaje completo, padres y archivos cambiados.
- Búsqueda de commits por mensaje, autor, correo o hash.
- Cuatro temas oscuros a elegir desde el menú "Opciones": Violeta, Bosque, Brasa y Exilio. La elección se
  recuerda.
- Visor de diffs con números de línea y colores, en vista unificada o dividida (lado a lado). Soporta
  archivos nuevos, borrados, renombrados y binarios.
- Vista de cambios sin commitear: archivos sin añadir, preparados para commit y en conflicto.
- Descartar los cambios de un archivo, previa confirmación: deshace modificaciones, restaura borrados y
  borra archivos nuevos.
- Añadir y quitar archivos del índice, de uno en uno o todos a la vez.
- Editor de commits con resumen, descripción y opción de amend.
- Crear ramas a partir del commit seleccionado (por defecto HEAD), con opción de cambiar a ellas.
- Cambiar entre todas las ramas locales y remotas desde un selector con filtro.
- Fetch, pull y push desde la barra del repositorio.
- Hacer checkout de cualquier commit desde su menú contextual (HEAD separado).
- Refresco manual, con F5 o automático al volver a la ventana.
- Varios repositorios abiertos a la vez en pestañas. Las pestañas abiertas se recuerdan entre sesiones.
- Buscador de repositorios integrado: navegar por carpetas, filtrar, buscar repositorios de forma
  recursiva y abrirlos con "Abrir repositorio".
- Lista de repositorios recientes.
- Actualización desde la propia aplicación, con verificación de firma.

## Instalación

Cada versión publica ejecutables en la página de Releases del repositorio; no hace falta instalar Rust.

- **Linux**: descarga `git-exile-linux-x86_64.tar.gz`, descomprímelo y ejecuta `./git-exile`. Para tenerlo
  en el menú de aplicaciones y con su icono (en Wayland es la única forma de que la ventana lo muestre):

  ```sh
  install -Dm755 git-exile ~/.local/bin/git-exile
  install -Dm644 git-exile.png ~/.local/share/icons/hicolor/256x256/apps/git-exile.png
  install -Dm644 git-exile.desktop ~/.local/share/applications/git-exile.desktop
  ```

- **Windows**: descarga `git-exile-windows-x86_64.zip`, descomprímelo y abre `git-exile.exe`. Necesita
  [Git para Windows](https://git-scm.com/download/win).

Para publicar una versión basta con subir una etiqueta (`git tag v0.1.0 && git push origin v0.1.0`):
el workflow `.github/workflows/release.yml` pasa los tests, compila ambos ejecutables, los firma y crea la
Release.

### Actualizaciones

Al arrancar, la aplicación consulta la última Release de GitHub. Si hay una versión posterior muestra un aviso
con el botón "Actualizar"; si no la hay o no hay conexión, no muestra nada. También se puede comprobar desde
"Opciones", con "Buscar actualizaciones". Las compilaciones de desarrollo no comprueban nada al arrancar.

La actualización reemplaza el ejecutable y pide reiniciar; las pestañas abiertas se conservan. Si el ejecutable
está en una carpeta sin permisos de escritura (como `/usr/bin`), falla y ofrece el enlace para descargarla a
mano. El repositorio debe ser público: la consulta se hace sin credenciales.

Los archivos de cada Release van firmados con [zipsign](https://github.com/Kijewski/zipsign), y la aplicación
solo instala los que coinciden con la clave pública `assets/update-signing.pub`. Para publicar hace falta el
secreto `UPDATE_SIGNING_KEY` en el repositorio, con la clave privada en base64:

```sh
zipsign gen-key priv.key pub.key   # solo una vez; pub.key va a assets/update-signing.pub
base64 -w0 priv.key                # valor del secreto
```

La clave privada no debe subirse al repositorio. Si se pierde, las instalaciones existentes no aceptarán
versiones firmadas con otra clave y habrá que actualizarlas a mano.

## Requisitos

- `git` instalado y disponible en el `PATH` (se recomienda 2.31 o superior).
- En Linux, las bibliotecas habituales de X11 o Wayland que necesita `eframe`.
- Para compilar desde el código, Rust 1.89 o superior.
- Opcional: las fuentes Inter y JetBrains Mono (`pacman -S inter-font ttf-jetbrains-mono`). Si están
  instaladas se usan automáticamente; si no, se usan Segoe UI y Consolas en Windows, y las fuentes por
  defecto de egui en Linux.

## Compilación y uso

```sh
cargo build --release
```

Abrir uno o varios repositorios, cada uno en su pestaña:

```sh
cargo run --release -- /ruta/al/repositorio /ruta/a/otro
```

Sin argumentos se restauran las pestañas de la última sesión. Si no hay ninguna, se muestra el buscador
de repositorios.

### Buscador de repositorios

El botón "+" de la barra de pestañas (o Ctrl+T) abre el buscador en una pestaña nueva:

- "Inicio" y "Subir" para moverse, o escribir una ruta y pulsar Enter.
- Doble clic en una carpeta para entrar en ella. Las carpetas que son repositorios git aparecen marcadas.
- "Filtrar carpetas" filtra por nombre la carpeta actual.
- "Buscar repositorios aquí" busca repositorios dentro de la carpeta actual (hasta 5 niveles, sin entrar
  en carpetas ocultas ni en `node_modules`, `target`, etc.).
- "Abrir repositorio" abre la carpeta seleccionada, o la actual si es (o está dentro de) un repositorio.

En el panel izquierdo están los repositorios abiertos y los recientes para abrirlos directamente.

### Ramas

- **Crear una rama**: botón "Nueva rama" de la barra del repositorio. Parte del commit seleccionado en la
  lista o, si no hay ninguno (o está seleccionada la fila de cambios sin commitear), de HEAD. También se
  puede hacer con clic derecho sobre un commit, "Crear rama aquí…". Por defecto se cambia a la rama nueva.
- **Cambiar de rama**: el botón "Rama: ..." abre la lista de ramas locales y remotas. Se puede filtrar
  escribiendo y pulsar Enter para cambiar a la primera que coincida. Al elegir una rama remota se cambia a
  la rama local del mismo nombre, o se crea una que la siga. Con clic derecho sobre un commit también se
  puede cambiar a las ramas que apuntan a él.
- **Moverse a un commit**: clic derecho sobre un commit, "Hacer checkout de este commit". HEAD queda
  separado (sin rama), y el botón de rama lo indica. Para volver basta con elegir una rama; para conservar
  commits hechos ahí, crea antes una rama con "Nueva rama".

Los cambios sin commitear se mantienen al cambiar de rama; si chocan con la rama de destino, git rechaza
el cambio y se muestra su mensaje.

### Sincronizar con el remoto

Los botones de la barra del repositorio usan tus credenciales de git (agente SSH, credential helper). La
aplicación no puede pedir contraseñas ni passphrases: si git las necesita, la operación falla o, en el caso
de SSH lanzando la aplicación desde una terminal, espera a que se escriban allí.

- **Fetch** baja las novedades de todos los remotos sin tocar tus ramas ni tus archivos.
- **Pull** trae los commits de la rama remota solo si la rama local puede avanzar sin combinar nada
  (`git pull --ff-only`, ignora `pull.rebase`). Si las ramas han divergido, no cambia nada y lo avisa: el merge
  o el rebase se hacen desde la terminal.
- **Push** sube la rama actual. Si no sigue a ninguna remota, la crea en `origin` (o en el primer remoto) y
  la deja seguida. Nunca fuerza: si el remoto tiene commits que no están aquí, se rechaza y pide hacer pull.

Un repositorio sin remotos muestra "Conectar remoto…": pega la dirección de un repositorio vacío creado en
GitHub, GitLab u otro servidor y se añadirá como `origin`. Nada se sube hasta pulsar Push.

### Descartar cambios

El icono de la papelera de cada archivo descarta sus cambios, tras pedir confirmación; no se puede deshacer.

- **En "Sin añadir"**: los archivos nuevos se borran del disco; los modificados y borrados vuelven a la
  versión preparada para commit, o a la del último commit si no hay ninguna.
- **En "Preparados para commit"**: el archivo vuelve a como está en el último commit, incluidos sus cambios
  sin añadir. Los archivos nuevos se borran y los renombrados recuperan su nombre.

Los archivos en conflicto no tienen esta opción. Otro repositorio git dentro de la carpeta tampoco se borra.

### Buscar commits

El campo "Buscar commit, autor o hash…" de la barra del repositorio (Ctrl+F) resalta los commits que
coinciden: el resto se atenúa, pero sigue en su sitio con el grafo para no perder el contexto. Mientras hay
una búsqueda se sigue cargando el historial hasta revisarlo entero.

El propio campo muestra el contador de resultados y las flechas para recorrerlos, que seleccionan cada
commit y dan la vuelta al llegar a un extremo. Con el foco en el buscador, Enter va al siguiente y
Mayús+Enter al anterior. Escape o la "x" vacían la búsqueda.

## Atajos de teclado

| Tecla | Acción |
|---|---|
| Flecha arriba / abajo | Moverse por la lista de commits |
| Re Pág / Av Pág | Saltar una página |
| Inicio / Fin | Ir al primer / último commit cargado |
| F5 | Refrescar el repositorio |
| Ctrl+F | Buscar commits |
| Ctrl+T | Nueva pestaña con el buscador de repositorios |
| Ctrl+W | Cerrar la pestaña activa |
| Ctrl+Tab / Ctrl+Shift+Tab | Pestaña siguiente / anterior |
| Ctrl+Enter | Crear el commit desde el editor |

Clic central sobre una pestaña también la cierra. En la vista de cambios, un doble clic sobre un archivo lo mueve entre "Sin añadir" y "Preparados para commit".

## Estructura del código

```
src/
├── main.rs              Arranque de la aplicación
├── app.rs               Pestañas, persistencia y reparto de mensajes
├── repo_tab.rs          Estado y vista de un repositorio abierto
├── browser.rs           Exploración de carpetas y búsqueda de repositorios
├── worker.rs            Hilo en segundo plano para las operaciones de git
├── update.rs            Búsqueda e instalación de actualizaciones
├── git/                 Acceso a git (no depende de la interfaz)
│   ├── runner.rs        Ejecución del binario git y manejo de errores
│   ├── repo.rs          Apertura del repositorio, rama actual
│   ├── log.rs           Lectura del historial
│   ├── graph.rs         Cálculo de carriles del grafo
│   ├── details.rs       Detalles de un commit
│   ├── diff.rs          Parser de diffs unificados
│   ├── branch.rs        Listar, crear y cambiar de rama, checkout de commits
│   ├── remote.rs        Remotos y servicio en el que están alojados
│   ├── sync.rs          Fetch, pull y push
│   ├── status.rs        Estado del working tree
│   ├── worktree.rs      Diffs de cambios sin commitear
│   └── commit.rs        Añadir/quitar del índice y crear commits
└── ui/                  Componentes de la interfaz
    ├── commit_list.rs   Lista de commits
    ├── graph_view.rs    Dibujo del grafo
    ├── commit_details.rs
    ├── diff_view.rs
    ├── status_panel.rs
    ├── commit_editor.rs
    ├── branch_dialog.rs Diálogo de nueva rama
    ├── connect_remote_dialog.rs Diálogo para conectar un remoto
    ├── update_bar.rs    Aviso de versión nueva y progreso de la actualización
    ├── theme.rs         Temas de color, fuentes y estilo global
    ├── icons.rs         Iconos vectoriales y botones
    └── repo_browser.rs  Buscador de repositorios
```

Las operaciones de git se ejecutan en un hilo aparte para que la interfaz no se bloquee. La salida de git
se lee en formatos pensados para programas (`--format` con separadores de control, `--porcelain=v2 -z`)
y se fuerza una configuración estable (`LC_ALL=C`, sin colores, prefijos `a/` y `b/` en los diffs) para que
el resultado no dependa de la configuración local.

## Tests

```sh
cargo test
```

Los tests del módulo `git` crean repositorios temporales y ejecutan git real sobre ellos.

## Licencia

Distribuido bajo la licencia MIT. Consulta el archivo `LICENSE`.

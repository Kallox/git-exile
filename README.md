# git-exile

Cliente gráfico de git escrito en Rust con [egui](https://github.com/emilk/egui).

Permite ver el historial de un repositorio, revisar los cambios de cada commit y crear commits nuevos.
No se conecta con GitHub, GitLab ni ningún otro servicio: ejecuta el binario `git` instalado en el sistema,
por lo que usa tu configuración de git tal cual (`user.name`, `user.email`, hooks, firma GPG/SSH,
credential helpers, etc.).

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
- Tres temas oscuros a elegir desde el menú "Opciones": Violeta, Bosque y Brasa. La elección se recuerda.
- Visor de diffs con números de línea y colores, en vista unificada o dividida (lado a lado). Soporta
  archivos nuevos, borrados, renombrados y binarios.
- Vista de cambios sin commitear: archivos sin añadir, preparados para commit y en conflicto.
- Descartar los cambios de un archivo, previa confirmación: deshace modificaciones, restaura borrados y
  borra archivos nuevos.
- Añadir y quitar archivos del índice, de uno en uno o todos a la vez.
- Editor de commits con resumen, descripción y opción de amend.
- Crear ramas a partir del commit seleccionado (por defecto HEAD), con opción de cambiar a ellas.
- Cambiar entre todas las ramas locales y remotas desde un selector con filtro.
- Hacer checkout de cualquier commit desde su menú contextual (HEAD separado).
- Refresco manual, con F5 o automático al volver a la ventana.
- Varios repositorios abiertos a la vez en pestañas. Las pestañas abiertas se recuerdan entre sesiones.
- Buscador de repositorios integrado: navegar por carpetas, filtrar, buscar repositorios de forma
  recursiva y abrirlos con "Abrir repositorio".
- Lista de repositorios recientes.

## Requisitos

- Rust 1.89 o superior.
- `git` instalado y disponible en el `PATH` (se recomienda 2.31 o superior).
- En Linux, las bibliotecas habituales de X11 o Wayland que necesita `eframe`.
- Opcional: las fuentes Inter y JetBrains Mono (`pacman -S inter-font ttf-jetbrains-mono`). Si están
  instaladas se usan automáticamente; si no, se usan las fuentes por defecto de egui.

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
├── git/                 Acceso a git (no depende de la interfaz)
│   ├── runner.rs        Ejecución del binario git y manejo de errores
│   ├── repo.rs          Apertura del repositorio, rama actual
│   ├── log.rs           Lectura del historial
│   ├── graph.rs         Cálculo de carriles del grafo
│   ├── details.rs       Detalles de un commit
│   ├── diff.rs          Parser de diffs unificados
│   ├── branch.rs        Listar, crear y cambiar de rama, checkout de commits
│   ├── remote.rs        Remotos y servicio en el que están alojados
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

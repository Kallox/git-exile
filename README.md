# git-exile

Cliente gráfico de git escrito en Rust con [egui](https://github.com/emilk/egui).

Permite ver el historial de un repositorio, revisar los cambios de cada commit y crear commits nuevos.
No se conecta con GitHub, GitLab ni ningún otro servicio: ejecuta el binario `git` instalado en el sistema,
por lo que usa tu configuración de git tal cual (`user.name`, `user.email`, hooks, firma GPG/SSH,
credential helpers, etc.).

## Funcionalidades

- Historial de todas las ramas, remotos y tags con grafo de ramas y merges.
- Etiquetas de ramas, ramas remotas, tags y HEAD en cada commit.
- Carga paginada del historial, pensada para repositorios grandes.
- Detalles del commit: autor, committer, mensaje completo, padres y archivos cambiados.
- Visor de diffs con números de línea y colores. Soporta archivos nuevos, borrados, renombrados y binarios.
- Vista de cambios sin commitear: archivos sin añadir, preparados para commit y en conflicto.
- Añadir y quitar archivos del índice, de uno en uno o todos a la vez.
- Editor de commits con resumen, descripción y opción de amend.
- Refresco manual, con F5 o automático al volver a la ventana.
- Lista de repositorios recientes.

## Requisitos

- Rust 1.89 o superior.
- `git` instalado y disponible en el `PATH` (se recomienda 2.31 o superior).
- En Linux, las bibliotecas habituales de X11 o Wayland que necesita `eframe`.

## Compilación y uso

```sh
cargo build --release
```

Abrir un repositorio concreto:

```sh
cargo run --release -- /ruta/al/repositorio
```

Sin argumentos se abre el último repositorio usado. También se puede abrir uno desde el botón "Abrir…"
o desde el menú "Recientes".

## Atajos de teclado

| Tecla | Acción |
|---|---|
| Flecha arriba / abajo | Moverse por la lista de commits |
| Re Pág / Av Pág | Saltar una página |
| Inicio / Fin | Ir al primer / último commit cargado |
| F5 | Refrescar el repositorio |
| Ctrl+Enter | Crear el commit desde el editor |

En la vista de cambios, un doble clic sobre un archivo lo mueve entre "Sin añadir" y "Preparados para commit".

## Estructura del código

```
src/
├── main.rs              Arranque de la aplicación
├── app.rs               Estado de la aplicación y distribución de paneles
├── worker.rs            Hilo en segundo plano para las operaciones de git
├── git/                 Acceso a git (no depende de la interfaz)
│   ├── runner.rs        Ejecución del binario git y manejo de errores
│   ├── repo.rs          Apertura del repositorio, rama actual
│   ├── log.rs           Lectura del historial
│   ├── graph.rs         Cálculo de carriles del grafo
│   ├── details.rs       Detalles de un commit
│   ├── diff.rs          Parser de diffs unificados
│   ├── status.rs        Estado del working tree
│   ├── worktree.rs      Diffs de cambios sin commitear
│   └── commit.rs        Añadir/quitar del índice y crear commits
└── ui/                  Componentes de la interfaz
    ├── commit_list.rs   Lista de commits
    ├── graph_view.rs    Dibujo del grafo
    ├── commit_details.rs
    ├── diff_view.rs
    ├── status_panel.rs
    └── commit_editor.rs
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

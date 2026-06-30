# Windows GUI Architecture

## Estructura de navegación

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            MainWindow.xaml                               │
│                         (WinUI 3 Window Container)                       │
├─────────────────────────────────────────────────────────────────────────┤
│                            MainView.xaml                                 │
│                      (Navigation & Header Container)                     │
├───────────────────┬─────────────────────────────────────────────────────┤
│  Navigation View  │                   Content Frame                       │
│  ─────────────    │                ─────────────────                     │
│ ┌─────────────┐  │  ┌──────────────────────────────────────────────┐    │
│ │   General   │◄───┤│  GeneralView                                  │    │
│ │             │  │  │  - Status indicator                          │    │
│ │ Connections │◄───┤│  - Start/Stop buttons                        │    │
│ │             │  │  │  - System info (port, fingerprint, version)  │    │
│ │   System    │◄───┤│                                               │    │
│ │             │  │  │  ┌──────────────────────────────────────┐    │    │
│ │   Update    │◄───┤│  │ ConnectionsView                       │    │    │
│ │             │  │  │  │ - Connect to peer                    │    │    │
│ └─────────────┘  │  │  │ - Incoming requests (TOFU)           │    │    │
│                  │  │  │ - Active sessions                    │    │    │
│                  │  │  │ - Known peers (with fingerprints)    │    │    │
│                  │  │  │ - Layout configuration               │    │    │
│                  │  │  │                                       │    │    │
│                  │  │  │ ┌──────────────────────────────────┐ │    │    │
│                  │  │  │ │ SystemView                        │ │    │    │
│                  │  │  │ │ - Clipboard toggle               │ │    │    │
│                  │  │  │ │                                   │ │    │    │
│                  │  │  │ │ ┌──────────────────────────────┐ │ │    │    │
│                  │  │  │ │ │ UpdateView                   │ │ │    │    │
│                  │  │  │ │ │ - Version display            │ │ │    │    │
│                  │  │  │ │ │ - Update button              │ │ │    │    │
│                  │  │  │ │ │                              │ │ │    │    │
│                  │  │  │ │ └──────────────────────────────┘ │ │    │    │
│                  │  │  │ └──────────────────────────────────┘ │    │    │
│                  │  │  └──────────────────────────────────────────┘    │
│                  │  └──────────────────────────────────────────────────┘
└───────────────────┴─────────────────────────────────────────────────────┘
```

## Flujo de datos

```
┌──────────────────────────────────────────────────────────────────────┐
│                         DaemonViewModel                               │
│                    (Punto central de estado)                          │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  ObservableObject Properties:                              │    │
│  │  • Connection (Connected/Connecting/Disconnected/...)      │    │
│  │  • StatusText                                              │    │
│  │  • Fingerprint, Port                                       │    │
│  │  • Capturing, ClipboardSharing                             │    │
│  │  • Sessions, Pending, Peers, Placements (Collections)      │    │
│  │  • DaemonVersion, LastError                                │    │
│  │                                                             │    │
│  │  Commands (async):                                         │    │
│  │  • StartDaemonAsync()                                      │    │
│  │  • StopDaemonAsync()                                       │    │
│  │  • ConnectAsync(host)                                      │    │
│  │  • DisconnectAsync(host)                                   │    │
│  │  • AcceptAsync(selector)                                   │    │
│  │  • RejectAsync(selector)                                   │    │
│  │  • SetLayoutAsync(host, edge)                              │    │
│  │  • SetClipboardAsync(enabled)                              │    │
│  │  • RemovePeerAsync(selector)                               │    │
│  └─────────────────────────────────────────────────────────────┘    │
└──────────────────────┬───────────────────────────────────────────────┘
                       │
         ┌─────────────┼─────────────┬──────────────┬────────────┐
         │             │             │              │            │
      ┌──▼──┐      ┌──▼──┐      ┌──▼──┐       ┌──▼──┐     ┌──▼──┐
      │Gen  │      │Conn │      │Sys  │       │Upd  │     │IPC  │
      │View │      │View │      │View │       │View │     │Clnt │
      └──────┘      └──────┘      └──────┘       └──────┘     └──────┘
         │             │             │              │            │
         └─────────────┼─────────────┴──────────────┴────────────┘
                       │
                ┌──────▼──────┐
                │  IPC Layer  │
                │  (Named Pipe)│
                └──────┬──────┘
                       │
            ┌──────────▼──────────┐
            │  Rust Daemon        │
            │  (omni-runtime)     │
            └─────────────────────┘
```

## Mapeo de vistas a responsabilidades

| Vista | Responsabilidad | Contiene |
|-------|-----------------|----------|
| **GeneralView** | Daemon control y info | Status, Start/Stop, info local, errores |
| **ConnectionsView** | Gestión de conexiones | Connect, pending, sesiones, peers, layout |
| **SystemView** | Configuración global | Clipboard toggle |
| **UpdateView** | Actualizaciones | Version, botón de update |

## Flujo de navegación

```
User clicks nav item
         │
         ▼
MainView.OnNavItemInvoked(NavigationViewItemInvokedEventArgs)
         │
         ▼
Extract tag from clicked item
         │
         ▼
NavigateToSection(tag: string)
         │
         ▼
Create view based on tag:
┌────────┴────────┬──────────────┬──────────────┬──────────┐
│                 │              │              │          │
"general"     "connections"  "system"      "update"        │
│                 │              │              │          │
▼                 ▼              ▼              ▼          ▼
GeneralView   ConnectionsView SystemView   UpdateView   Other
         │
         ▼
ContentFrame.Content = view instance
         │
         ▼
View renders with ViewModel bindings
```

## Binding y reactividad

```
DaemonViewModel
      │
      ├─▶ PropertyChanged event fires
      │
      ├─▶ x:Bind Mode=OneWay en Views
      │   (Automático a través del binding)
      │
      ├─▶ Collections (ObservableCollection)
      │   └─▶ ItemsControl se actualiza automáticamente
      │
      ├─▶ Command handlers en code-behind
      │   └─▶ await ViewModel.ConnectAsync(...)
      │
      └─▶ Back to daemon via IPC client
```

## Jerarquía de componentes

```
App
 └─ MainWindow
     └─ MainView
         ├─ Header (Title + Status)
         └─ NavigationView
             ├─ MenuItems (General, Connections, System, Update)
             └─ Frame
                 └─ Current View
                     ├─ GeneralView
                     ├─ ConnectionsView
                     ├─ SystemView
                     └─ UpdateView
```

## Comparación con macOS

```
macOS NavigationSplitView:              Windows NavigationView:
┌─────────┬──────────────┐              ┌──────────┬─────────────┐
│ Sidebar │              │              │   Nav    │             │
│         │  Detail Pane │              │   Menu   │  Content    │
├─────────┤              │              ├──────────┤             │
│General  │ (swaps views)│              │General   │ (swaps      │
│         │              │              │Conn.     │  views)     │
│Conn.    │              │              │System    │             │
│         │              │              │Update    │             │
│System   │              │              │          │             │
│         │              │              │          │             │
│Update   │              │              │          │             │
│         │              │              │          │             │
└─────────┴──────────────┘              └──────────┴─────────────┘
```

## Puntos clave de diseño

### 1. **Single Responsibility Principle**
- Cada vista maneja una sección lógica
- MainView solo orquesta navegación
- DaemonViewModel centraliza lógica de negocio

### 2. **Reactive Data Binding**
- ObservableObject con PropertyChanged
- x:Bind OneWay desde XAML
- Colecciones automáticamente refrescadas

### 3. **Separación de concerns**
- IPC logic en Omni.Ipc
- UI state en Omni.App.Core
- Rendering en Views/

### 4. **Escalabilidad**
- Agregar vista nueva = copiar una existente y registrar en MainView
- Agregar campo al ViewModel = automáticamente disponible en todas las vistas

### 5. **Testability**
- Cada vista puede instantiarse con un mock de ViewModel
- Commands son métodos async públicos
- Sin lógica UI en code-behind (handlers simplemente llaman al ViewModel)

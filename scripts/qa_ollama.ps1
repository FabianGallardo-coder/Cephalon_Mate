<#
.SYNOPSIS
    QA end-to-end de Cephalon Mate con Ollama real.

.DESCRIPTION
    Lanza el binario en modo headless (--ask), que usa EXACTAMENTE el mismo
    flujo que la TUI (prefetch por intención + loop de tools), y comprueba que
    la respuesta del modelo se apoya en los datos reales de WFCD y que no
    inventa drops, polaridades ni recetas.

    Cada pregunta tiene dos listas:
      - "esperar": textos que DEBEN aparecer en la respuesta.
      - "prohibir": textos que NO deben aparecer (alucinaciones conocidas).

.EXAMPLE
    pwsh -File scripts/qa_ollama.ps1
    pwsh -File scripts/qa_ollama.ps1 -Modelo "cehalon-ordis:latest"
#>
param(
    [string]$Modelo = "",
    [string]$HostOllama = "http://127.0.0.1:12434",
    [string]$Binario = ".\target\debug\cehalon-mate.exe"
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

if (-not (Test-Path $Binario)) {
    Write-Output "No existe $Binario — ejecuta 'cargo build' primero."
    exit 1
}

# Comprueba que Ollama responde.
try {
    $tags = Invoke-RestMethod -Uri "$HostOllama/api/tags" -TimeoutSec 8
    Write-Output "Ollama OK en $HostOllama ($($tags.models.Count) modelos)"
} catch {
    Write-Output "Ollama no disponible en $HostOllama : $($_.Exception.Message)"
    exit 1
}

# Preguntas de QA:
#   esperar   → textos que DEBEN aparecer (datos verificados)
#   opcional  → grupo donde basta con que aparezca UNO (tolerancia a la
#               varianza del modelo al elegir qué fuentes citar)
#   prohibido → textos que NO deben aparecer (alucinaciones conocidas)
$preguntas = @(
    @{
        q = "¿Dónde dropea Serration y con qué probabilidad?"
        esperar = @("Nauseous Crawler", "%")
        opcional = @("Scorpion", "Rotation", "Ceres", "Void Shade")
        prohibido = @("Charger", "860%")
        nota = "drops reales de Serration con probabilidad"
    },
    @{
        q = "Háblame de Rhino: polaridades, stats y habilidades"
        esperar = @("vazarin", "Rhino Charge")
        opcional = @("Iron Skin", "Roar", "Rhino Stomp", "Vida")
        prohibido = @("Láser", "Impacto")
        nota = "polaridad y habilidades reales de Rhino"
    },
    @{
        q = "¿Dónde consigo Arrow Mutation y qué polaridad tiene?"
        esperar = @("naramon")
        opcional = @("Duviri", "Kuva Fortress", "Ceres", "Rotation")
        prohibido = @()
        nota = "polaridad y drop del mod de Duviri"
    },
    @{
        q = "¿Qué mods van en Nova?"
        esperar = @("madurai")
        opcional = @("Accelerated Blast", "Amanata", "Adhesive", "Adrenaline")
        prohibido = @("vazarin")
        nota = "mods compatibles según la polaridad de Nova"
    },
    @{
        q = "¿Dónde dropea el Hornet Prime y qué materiales necesita para craftearlo?"
        esperar = @()
        opcional = @("no tiene", "no está", "NO CATALOGADO", "no disponible", "no catalogado")
        prohibido = @("Orokin Cell", "Neurode ×1")
        nota = "item ausente del dataset: no debe inventar receta"
    }
)

$total = 0
$ok = 0

foreach ($p in $preguntas) {
    $total++
    Write-Output ""
    Write-Output "============================================================"
    Write-Output "PREGUNTA: $($p.q)"
    Write-Output "  (nota: $($p.nota))"

    # Flujo real de la app: --ask usa el mismo camino que la TUI.
    $args = @("--ask", $p.q)
    if ($Modelo) { $args = @("--model", $Modelo) + $args }
    $salida = & $Binario @args 2>&1 | Out-String

    Write-Output "  --- respuesta ---"
    Write-Output ("  " + ($salida.Trim() -replace "`r?`n", "`n  "))

    $fallos = @()
    foreach ($e in $p.esperar) {
        if ($salida -notmatch [regex]::Escape($e)) {
            $fallos += "falta '$e'"
        }
    }
    # Grupo opcional: basta con que aparezca uno de los textos.
    $opcionales = @($p.opcional)
    if ($opcionales.Count -gt 0) {
        $alguno = $false
        foreach ($o in $opcionales) {
            if ($salida -match [regex]::Escape($o)) { $alguno = $true; break }
        }
        if (-not $alguno) {
            $fallos += "no cita ninguna fuente real (esperaba una de: $($opcionales -join ', '))"
        }
    }
    foreach ($x in $p.prohibir) {
        if ($salida -match [regex]::Escape($x)) {
            $fallos += "alucinó '$x'"
        }
    }
    # La respuesta debe pasar por el prefetch (tool visible en la salida).
    if ($salida -notmatch "\[tool\]") {
        $fallos += "no se ejecutó ninguna tool (prefetch)"
    }

    if ($fallos.Count -eq 0) {
        $ok++
        Write-Output "  ✔ QA OK"
    } else {
        Write-Output "  ✘ QA FAIL — $($fallos -join '; ')"
    }
}

Write-Output ""
Write-Output "============================================================"
Write-Output "QA OLLAMA: $ok/$total preguntas correctas"
if ($ok -eq $total) { exit 0 } else { exit 1 }
